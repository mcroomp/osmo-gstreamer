//! GStreamer plugin exposing the `osmosrc` source scaffold.
//!
//! The element advertises Annex-B H.264 access units but cannot stream yet.

use gst::prelude::*;
use gstreamer as gst;

mod source;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), gst::glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "osmosrc",
        gst::Rank::NONE,
        source::OsmoSrc::static_type(),
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    fn init() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            gst::init().expect("GStreamer initialization");
            plugin_register_static().expect("plugin registration");
        });
    }

    #[test]
    fn registers_source_with_h264_caps() {
        init();
        let source = gst::ElementFactory::make("osmosrc")
            .build()
            .expect("source factory");
        let pad = source.static_pad("src").expect("source pad");
        let caps = pad.pad_template_caps();
        let structure = caps.structure(0).expect("source caps");
        assert_eq!(structure.name(), "video/x-h264");
        assert_eq!(
            structure.get::<String>("stream-format").unwrap(),
            "byte-stream"
        );
        assert_eq!(structure.get::<String>("alignment").unwrap(), "au");
    }

    #[test]
    fn streaming_fails_explicitly_until_implemented() {
        init();
        let pipeline = gst::Pipeline::new();
        let source = gst::ElementFactory::make("osmosrc").build().unwrap();
        pipeline.add(&source).unwrap();
        let result = pipeline.set_state(gst::State::Paused);
        let message = pipeline
            .bus()
            .unwrap()
            .timed_pop_filtered(gst::ClockTime::from_seconds(1), &[gst::MessageType::Error]);
        pipeline.set_state(gst::State::Null).unwrap();
        assert!(result.is_err());
        let message = message.expect("explicit bus error");
        let gst::MessageView::Error(error) = message.view() else {
            panic!("expected an error message");
        };
        assert!(error.error().message().contains("not implemented"));
    }
}
