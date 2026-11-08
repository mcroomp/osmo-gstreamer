use crate::{Error, Result};
use uuid::Uuid;

pub(crate) const IDENTITY_LENGTH: usize = 32;

pub(crate) fn new_identity() -> String {
    Uuid::new_v4().simple().to_string()
}

pub(crate) fn validate_identity(identity: &str) -> Result<()> {
    let value = identity.trim();
    if value.len() != IDENTITY_LENGTH || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::InvalidConfig {
            field: "pairing identity",
            reason: "expected a 32-hex-character installation identity",
        });
    }
    Ok(())
}
