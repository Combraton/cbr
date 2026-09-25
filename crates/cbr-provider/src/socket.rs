//! The Unix-domain-socket form of the stream binding (STREAM section 6).
//!
//! Framing, frame-level failures and the JSON-RPC mapping are the stdio form's;
//! what this adds is the part that makes a transport many processes can reach
//! safe to share:
//!
//! - **Placement.** A pathname socket with mode `0600`, in a directory owned by
//!   this user, not a symbolic link, with no group or other permissions.
//!   Anything else refuses to start, before listening.
//! - **Peer check.** Each accepted connection's peer effective user id is read
//!   from the operating system (`SO_PEERCRED` on Linux, `getpeereid` on
//!   macOS). A different user is closed without a frame read or written.
//! - **Authentication.** Every session starts unauthenticated (CORE section
//!   18.2), so the principal comes from a credential, not from the launch.
//! - **Concurrency.** One thread per connection, each with its own outbox and
//!   its own store connection, all serialized by the provider's processing
//!   lock.
//!
//! **What the peer check establishes is "same operating-system user".** The
//! different-user branch cannot be exercised on a machine without a second
//! user it can act as: Protocol's own CI tests it by connecting as root through
//! passwordless `sudo`, which is the only other user it tests. Where that is
//! unavailable it is a coverage limit, never a pass.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{io, io::Read};

use crate::clock::Clock;
use crate::config::Config;
use crate::provider::Provider;
use crate::provider::Serving;
use crate::session;
use crate::work::Pool;

/// How often an idle session wakes to re-check its subscriptions and deliver
/// events other sessions committed. Well inside the 2-second bound CORE
/// section 16.5 gives lapses caused elsewhere.
const POLL: Duration = Duration::from_millis(40);

/// A socket reader that also wakes when this session's FIFO processing ticket
/// reaches the front. The ordinary timeout remains the cadence for
/// opportunistic maintenance and subscription checks when nothing is queued.
struct PollingStream {
    socket: UnixStream,
    wake: UnixStream,
}

impl Read for PollingStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            let mut descriptors = [
                libc::pollfd {
                    fd: self.socket.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.wake.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            // SAFETY: `descriptors` is a live array of two pollfd values for
            // the duration of the call; poll changes only their `revents`.
            let ready = unsafe {
                libc::poll(
                    descriptors.as_mut_ptr(),
                    descriptors.len() as libc::nfds_t,
                    POLL.as_millis() as libc::c_int,
                )
            };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready == 0 {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            let socket_events = descriptors[0].revents;
            if socket_events & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                return self.socket.read(buffer);
            }
            if descriptors[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                crate::barriers::signal(crate::barriers::IDLE_PROCESSING_WOKEN);
                let mut wake = [0u8; 64];
                loop {
                    match self.wake.read(&mut wake) {
                        Ok(0) => break,
                        Ok(_) => {}
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                        Err(error) => return Err(error),
                    }
                }
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
        }
    }
}

/// Refuse a socket directory another user could reach into (STREAM section 6).
pub fn check_directory(socket: &Path) -> Result<(), String> {
    let directory = socket.parent().ok_or("the socket path has no directory")?;
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|error| format!("socket directory {}: {error}", directory.display()))?;
    let safe = metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == effective_uid()
        && metadata.mode() & 0o077 == 0;
    if safe {
        Ok(())
    } else {
        Err(format!(
            "unsafe socket directory {}: it must be a directory, not a symbolic link, owned by \
             this user, with no group or other permissions",
            directory.display()
        ))
    }
}

/// Listen, and serve each connection on its own thread, until standard input
/// ends. The directory is checked before anything is bound.
pub fn serve(
    socket: &Path,
    config: Config,
    clock: Arc<Clock>,
    work: Arc<Pool>,
    model: Option<Arc<Serving>>,
    data_dir: PathBuf,
) -> Result<(), String> {
    check_directory(socket)?;
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).map_err(|error| format!("bind: {error}"))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("socket mode: {error}"))?;

    // How the process is started and stopped is outside the binding; the
    // conformance launch ties it to standard input, and so does CBR.
    std::thread::spawn(|| {
        let mut sink = Vec::new();
        let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut sink);
        std::process::exit(0);
    });

    let own = effective_uid();
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if peer_uid(&stream) != Some(own) {
            // A different user: closed without reading or writing a frame.
            continue;
        }
        let (config, clock, data_dir) = (config.clone(), clock.clone(), data_dir.clone());
        let work = work.clone();
        let model = model.clone();
        std::thread::spawn(move || {
            if let Err(error) = serve_connection(stream, config, clock, work, model, &data_dir) {
                eprintln!("cbr-provider: session: {error}");
            }
        });
    }
    Ok(())
}

fn serve_connection(
    stream: UnixStream,
    config: Config,
    clock: Arc<Clock>,
    work: Arc<Pool>,
    model: Option<Arc<Serving>>,
    data_dir: &Path,
) -> Result<(), String> {
    let mut provider = Provider::connect(config, clock, data_dir)
        .map_err(|error| format!("opening the store: {error}"))?
        .with_work(work)
        .with_model(model);
    let writer = stream.try_clone().map_err(|error| error.to_string())?;
    let closer = stream.try_clone().map_err(|error| error.to_string())?;
    let (wake_reader, wake_writer) = UnixStream::pair().map_err(|error| error.to_string())?;
    wake_reader
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let wake = Arc::new(
        crate::provider::ProcessingWake::new(wake_writer).map_err(|error| error.to_string())?,
    );
    provider.set_processing_wake(wake);
    let input = PollingStream {
        socket: stream,
        wake: wake_reader,
    };
    match session::serve(input, writer, &mut provider, Some(&closer))? {
        session::Ended::Normally => {}
        session::Ended::TooSlow(record) => eprintln!("cbr-provider: {}", record.record()),
    }
    let _ = closer.shutdown(std::net::Shutdown::Both);
    Ok(())
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: the buffer and length describe a valid `ucred` for SO_PEERCRED.
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    (status == 0).then_some(credentials.uid)
}

#[cfg(not(target_os = "linux"))]
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: getpeereid writes two integers through valid pointers.
    let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    (status == 0).then_some(uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_socket_directory_other_users_can_reach_is_refused() {
        let directory = tempfile::tempdir().expect("temp dir");
        let socket = directory.path().join("provider.sock");

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        assert!(check_directory(&socket).is_ok(), "0700 and owned: accepted");

        for mode in [0o750, 0o705, 0o770, 0o777] {
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(mode))
                .expect("chmod");
            assert!(
                check_directory(&socket).is_err(),
                "mode {mode:o} grants group or other access and must be refused"
            );
        }

        // A symbolic link to a safe directory is still refused: the link
        // could be repointed after the check.
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let outer = tempfile::tempdir().expect("temp dir");
        let link = outer.path().join("link");
        std::os::unix::fs::symlink(directory.path(), &link).expect("symlink");
        assert!(check_directory(&link.join("provider.sock")).is_err());
    }

    #[test]
    fn the_peer_of_a_same_user_connection_is_this_user() {
        // The positive control for the peer check. Its negative — a
        // different user refused — needs a second user this process can act
        // as, and is a coverage limit where there is none.
        let (left, _right) = UnixStream::pair().expect("pair");
        assert_eq!(peer_uid(&left), Some(effective_uid()));
    }
}
