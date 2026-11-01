use gstreamer as gst;
use gstreamer_base as gst_base;
use std::sync::OnceLock;

mod source;

fn init_logging() {
    static LOGGER: OnceLock<gst::log::DebugCategoryLogger> = OnceLock::new();
    let logger = LOGGER.get_or_init(|| {
        gst::log::DebugCategoryLogger::new(gst::DebugCategory::new(
            "osmo-camera",
            gst::DebugColorFlags::empty(),
            Some("Unofficial Osmo camera interoperability"),
        ))
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
        log::info!("Forwarding Rust log records to GStreamer category osmo-camera");
    }
}

fn plugin_init(plugin: &gst::Plugin) -> Result<(), gst::glib::BoolError> {
    init_logging();
    gst::Element::register(
        Some(plugin),
        "osmosrc",
        gst::Rank::NONE,
        source::OsmoSrc::static_type(),
    )
}

use gst::prelude::*;
gst::plugin_define!(
    osmo,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    env!("CARGO_PKG_VERSION"),
    "MIT",
    env!("CARGO_PKG_NAME"),
    "osmo-gstreamer",
    "https://github.com/mcroomp/osmo-gstreamer"
);
