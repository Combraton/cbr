//! Issuing and handing off principal credentials (CORE section 18.1).
//!
//! A credential is `ccred1.<principal>.<secret>`, the secret 32 bytes from the
//! operating system's random source as unpadded base64url. The store keeps only
//! its digest. The credential itself exists in exactly one place: a **handoff
//! file** with mode `0600`, followed by one line feed, inside a directory with
//! mode `0700` owned by this user. It is never logged, never printed, never put
//! on a command line and never passed through the environment.

use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::store::Store;

/// Where a principal's credential is handed off for local callers.
pub fn handoff_path(data_dir: &Path, principal: &str) -> PathBuf {
    data_dir.join("credentials").join(principal)
}

/// A new credential for `principal`.
fn generate(principal: &str) -> Result<String, String> {
    let mut secret = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut secret))
        .map_err(|error| format!("reading the random source: {error}"))?;
    let url_safe: String = cbr_encoding::encode_base64(&secret)
        .trim_end_matches('=')
        .chars()
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            other => other,
        })
        .collect();
    Ok(format!("ccred1.{principal}.{url_safe}"))
}

/// Issue a credential for `principal`, revoking any earlier one, record its
/// digest, and write the handoff file. The file is written before the digest
/// is recorded, so a crash between them leaves a file that does not work —
/// which the next start replaces — never a working credential nobody holds.
pub fn issue(store: &mut Store, data_dir: &Path, principal: &str) -> Result<PathBuf, String> {
    if principal.contains(['/', '.']) || principal.is_empty() {
        return Err(format!(
            "principal {principal:?} cannot name a credential file"
        ));
    }
    let credential = generate(principal)?;
    let directory = data_dir.join("credentials");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let path = handoff_path(data_dir, principal);
    let staged = directory.join(format!(".{principal}.staging"));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&staged)
            .map_err(|error| error.to_string())?;
        writeln!(file, "{credential}").map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    std::fs::rename(&staged, &path).map_err(|error| error.to_string())?;
    std::fs::File::open(&directory)
        .and_then(|d| d.sync_all())
        .map_err(|error| error.to_string())?;
    store
        .rotate_credential(principal, &cbr_encoding::sha256_hex(credential.as_bytes()))
        .map_err(|error| error.to_string())?;
    Ok(path)
}

/// Make sure `principal` has a live credential whose handoff file exists. The
/// store holds only digests, so a credential whose file is gone cannot be
/// recovered, only replaced.
pub fn ensure(store: &mut Store, data_dir: &Path, principal: &str) -> Result<(), String> {
    let path = handoff_path(data_dir, principal);
    let live = std::fs::read_to_string(&path).ok().is_some_and(|text| {
        let digest = cbr_encoding::sha256_hex(text.trim_end_matches('\n').as_bytes());
        store.credentials().is_ok_and(|stored| {
            stored
                .iter()
                .any(|c| c.digest == digest && c.principal == principal && !c.revoked)
        })
    });
    if live {
        return Ok(());
    }
    issue(store, data_dir, principal).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn a_handoff_file_is_private_and_only_its_digest_is_stored() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut store = Store::open(directory.path()).expect("store");
        let path = issue(&mut store, directory.path(), "owner").expect("issues");

        let written = std::fs::read_to_string(&path).expect("reads");
        assert!(written.ends_with('\n') && written.starts_with("ccred1.owner."));
        let credential = written.trim_end_matches('\n');
        assert_eq!(credential.len(), "ccred1.owner.".len() + 43);

        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );

        let stored = store.credentials().expect("credentials");
        assert_eq!(stored.len(), 1);
        assert_eq!(
            stored[0].digest,
            cbr_encoding::sha256_hex(credential.as_bytes())
        );
        assert!(
            !stored.iter().any(|c| c.digest.contains(credential)),
            "the credential itself is never stored"
        );

        // Rotation leaves exactly one live credential, and it is the new one.
        issue(&mut store, directory.path(), "owner").expect("rotates");
        let after = store.credentials().expect("credentials");
        assert_eq!(after.iter().filter(|c| !c.revoked).count(), 1);
        let rotated = std::fs::read_to_string(&path).expect("reads");
        assert_ne!(rotated, written);
    }
}
