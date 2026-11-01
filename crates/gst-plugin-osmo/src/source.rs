use gst::{glib, subclass::prelude::*};
use gst_base::{prelude::*, subclass::prelude::*};
use gstreamer as gst;
use gstreamer_base as gst_base;
use std::sync::LazyLock;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct OsmoSrc;

    #[glib::object_subclass]
    impl ObjectSubclass for OsmoSrc {
        const NAME: &'static str = "GstOsmoSrc";
        type Type = super::OsmoSrc;
        type ParentType = gst_base::PushSrc;
    }

    impl ObjectImpl for OsmoSrc {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_live(true);
            self.obj().set_format(gst::Format::Time);
        }
    }

    impl GstObjectImpl for OsmoSrc {}

    impl ElementImpl for OsmoSrc {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
                gst::subclass::ElementMetadata::new(
                    "Osmo camera source",
                    "Source/Video",
                    "Unofficial interoperability source; streaming is not implemented",
                    "osmo-gstreamer contributors",
                )
            });
            Some(&METADATA)
        }

        fn pad_templates() -> &'static [gst::PadTemplate] {
            static TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
                let caps = gst::Caps::builder("video/x-h264")
                    .field("stream-format", "byte-stream")
                    .field("alignment", "au")
                    .build();
                vec![
                    gst::PadTemplate::new(
                        "src",
                        gst::PadDirection::Src,
                        gst::PadPresence::Always,
                        &caps,
                    )
                    .expect("valid H.264 pad template"),
                ]
            });
            &TEMPLATES
        }
    }

    impl BaseSrcImpl for OsmoSrc {
        fn start(&self) -> Result<(), gst::ErrorMessage> {
            Err(gst::error_msg!(
                gst::ResourceError::NotFound,
                ("Camera streaming is not implemented"),
                ["osmo-camera {}", osmo_camera::version()]
            ))
        }
    }

    impl PushSrcImpl for OsmoSrc {}
}

glib::wrapper! {
    pub struct OsmoSrc(ObjectSubclass<imp::OsmoSrc>)
        @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}
