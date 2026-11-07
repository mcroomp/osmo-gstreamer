//! Wi-Fi network join payload.

use super::{cmd_07_07_ssid::MAX_SSID_BYTES, packed_string::write_packed_string};
use crate::{Error, protocol::ResponseStatus};
use binrw::BinWrite;
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Encodes the SSID and password as consecutive length-prefixed UTF-8 strings.
///
/// APP -> Wifi | 0x07/0x47
///
/// ```text
/// request {
///     ssid_length: u8
///     ssid: [u8; ssid_length] | UTF-8
///     password_length: u8
///     password: [u8; password_length] | UTF-8, sensitive
/// }
/// reply { status: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// The Nano request accepts 1..=32 encoded SSID bytes and either an empty
/// password or 8..=63 encoded password bytes. Limits count bytes, not characters.
/// Length prefixes do not include a terminating NUL.
///
/// Replies reuse `CameraReadReply<RemainingBytes>`. A nonempty all-zero payload
/// indicates acceptance; status 0x01 followed by 0xFF qualifies the existing
/// bounded retry policy, not a uniquely temporary refusal.
/// Status-zero replies with nonzero trailing bytes are not accepted
/// by the lifecycle. Other detail bytes remain uninterpreted. Acceptance is
/// not proof of association, IP configuration or video readiness.
#[derive(BinWrite)]
#[bw(assert(!ssid.is_empty() && ssid.len() <= MAX_SSID_BYTES, Error::InvalidConfig {
    field: "Wi-Fi SSID", reason: "must contain 1..=32 UTF-8 bytes",
}))]
#[bw(assert(password.is_empty() || (8..=63).contains(&password.len()), Error::InvalidConfig {
    field: "Wi-Fi password", reason: "must be empty or contain 8..=63 UTF-8 bytes",
}))]
#[bw(magic = b"\x07\x47")]
pub struct JoinNetworkRequest<'a> {
    #[bw(write_with = write_packed_string)]
    pub ssid: &'a str,
    #[bw(write_with = write_packed_string)]
    pub password: &'a str,
}

/// Status and first detail byte used by the host's bounded retry policy.
/// These bytes do not establish that a failure is transient or retry-safe.
pub const JOIN_RETRY_STATUS: ResponseStatus = ResponseStatus::Unknown(1);
pub const JOIN_RETRY_DETAIL: u8 = 0xff;

impl Debug for JoinNetworkRequest<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("JoinNetworkRequest { credentials: [REDACTED] }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandPacket;
    use std::io::Cursor;

    #[test]
    fn join_lengths_are_checked_before_writing_either_string() {
        for ssid in ["x".to_owned(), "x".repeat(32), "\u{00e9}".repeat(16)] {
            for password in ["".to_owned(), "p".repeat(8), "p".repeat(63)] {
                let bytes = CommandPacket::payload(&JoinNetworkRequest {
                    ssid: &ssid,
                    password: &password,
                })
                .unwrap();
                assert_eq!(usize::from(bytes[0]), ssid.len());
                assert_eq!(usize::from(bytes[1 + ssid.len()]), password.len());
                assert_eq!(bytes.len(), ssid.len() + password.len() + 2);
            }
        }
        for (ssid, password) in [
            ("".to_owned(), String::new()),
            ("x".repeat(33), String::new()),
            ("\u{00e9}".repeat(17), String::new()),
            ("x".to_owned(), "p".repeat(7)),
            ("x".to_owned(), "p".repeat(64)),
        ] {
            let mut writer = Cursor::new(Vec::new());
            assert!(
                CommandPacket::payload(&JoinNetworkRequest {
                    ssid: &ssid,
                    password: &password
                })
                .map(|bytes| writer.get_mut().extend(bytes))
                .is_err()
            );
            assert!(writer.into_inner().is_empty());
        }
    }
}
