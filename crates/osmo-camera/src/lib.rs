//! Native foundation for Osmo camera integration.
//!
//! Camera protocol, transport and connection lifecycle APIs are not yet implemented.
//! Python and GStreamer integrations depend on this crate, not on each other.

/// Returns the native library version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_package_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
