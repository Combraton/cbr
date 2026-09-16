//! The stream binding's frame layer (STREAM sections 1 and 2).
//!
//! A frame is bytes terminated by one line feed; the terminator is not part of
//! the frame. Two rules here are load-bearing and easy to get subtly wrong:
//! a receiver must never buffer more than limit + 1 bytes of a frame it has not
//! finished reading, and bytes after the last terminator at end of input are
//! discarded without being parsed.

use std::io::Read;

use crate::errors::FrameFailure;

pub const PRE_NEGOTIATION_LIMIT: usize = 1_048_576;

pub enum Frame {
    /// A frame with content to process.
    Bytes(Vec<u8>),
    /// Empty, or only insignificant whitespace. Ignored (STREAM section 1.4).
    Blank,
    /// The frame exceeded the receiver's limit.
    TooLarge,
    /// Input ended. Any bytes after the last terminator were discarded.
    Eof,
    /// No complete frame arrived before the connection's read timeout. Bytes
    /// of a partly received frame are kept for the next call. Only a
    /// connection with a read timeout — the socket form, which polls so idle
    /// sessions are re-checked — ever sees this.
    Idle,
}

pub struct FrameReader<R: Read> {
    input: R,
    buffered: Vec<u8>,
    at: usize,
    eof: bool,
    /// The frame being assembled. Held here rather than in `next_frame`, so a
    /// read timeout in the middle of a frame does not discard what arrived.
    partial: Vec<u8>,
}

impl<R: Read> FrameReader<R> {
    pub fn new(input: R) -> Self {
        Self {
            input,
            buffered: Vec::new(),
            at: 0,
            eof: false,
            partial: Vec::new(),
        }
    }

    /// Read the next frame under `limit`, which may change between frames:
    /// the binding uses 1 MiB until negotiation completes, then the value each
    /// side advertised.
    pub fn next_frame(&mut self, limit: usize) -> std::io::Result<Frame> {
        loop {
            if self.at == self.buffered.len() {
                if self.eof {
                    // Bytes after the last terminator are discarded without
                    // being parsed, so an unterminated tail is simply dropped.
                    return Ok(Frame::Eof);
                }
                self.buffered.clear();
                self.buffered.resize(8192, 0);
                let read = match self.input.read(&mut self.buffered) {
                    Ok(read) => read,
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        self.buffered.clear();
                        self.at = 0;
                        return Ok(Frame::Idle);
                    }
                    Err(error) => return Err(error),
                };
                self.buffered.truncate(read);
                self.at = 0;
                if read == 0 {
                    self.eof = true;
                    continue;
                }
            }
            let chunk = &self.buffered[self.at..];
            match chunk.iter().position(|byte| *byte == b'\n') {
                Some(offset) => {
                    let mut frame = std::mem::take(&mut self.partial);
                    frame.extend_from_slice(&chunk[..offset]);
                    self.at += offset + 1;
                    if frame.len() > limit {
                        return Ok(Frame::TooLarge);
                    }
                    if frame
                        .iter()
                        .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
                    {
                        return Ok(Frame::Blank);
                    }
                    return Ok(Frame::Bytes(frame));
                }
                None => {
                    self.partial.extend_from_slice(chunk);
                    self.at = self.buffered.len();
                    // Stop accumulating as soon as the limit is passed. The
                    // remaining bytes of an oversized frame are never buffered,
                    // and the connection closes, so they are never read.
                    if self.partial.len() > limit {
                        return Ok(Frame::TooLarge);
                    }
                }
            }
        }
    }
}

/// One outgoing frame: canonical bytes, then the terminator. Never a line feed
/// inside, which canonical encoding guarantees by escaping control characters.
/// Its length is what the pending-output bound counts.
pub fn encode(value: &cbr_encoding::Value) -> Vec<u8> {
    let mut bytes = cbr_encoding::to_canonical(value);
    debug_assert!(
        !bytes.contains(&b'\n'),
        "a frame must not contain a line feed"
    );
    bytes.push(b'\n');
    bytes
}

/// Classify a frame's bytes, applying the domain's I-JSON rules.
///
/// Returns the parsed value, or the frame-level failure that requires closing
/// the connection.
pub fn parse_frame(bytes: &[u8]) -> Result<cbr_encoding::Value, FrameFailure> {
    match cbr_encoding::parse(bytes) {
        Ok(value) => Ok(value),
        Err(error) => Err(match error.kind {
            cbr_encoding::ErrorKind::InvalidUtf8 => FrameFailure::InvalidUtf8,
            // Everything else is "not a JSON text, or violates I-JSON", which
            // the binding reports as parse_error. That includes a byte-order
            // mark, a duplicate member, a non-integer number and a noncharacter.
            _ => FrameFailure::ParseError,
        }),
    }
}
