//! What a desktop's default device stands for, read from PipeWire's metadata as
//! `pw-metadata` prints it.

use summit_capture::standing_for;

/// What `pw-metadata -n default 0` prints, whole: the configured defaults, spaced, come first.
const PRINTED: &str = r#"Found "default" metadata 40
update: id:0 key:'default.configured.audio.sink' value:'{ "name": "alsa_output.usb-Headset-00.analog-stereo" }' type:'Spa:String:JSON'
update: id:0 key:'default.configured.audio.source' value:'{ "name": "alsa_input.usb-Headset-00.mono-fallback" }' type:'Spa:String:JSON'
update: id:0 key:'default.audio.sink' value:'{"name":"alsa_output.pci-0000_00_1f.3.analog-stereo"}' type:'Spa:String:JSON'
update: id:0 key:'default.audio.source' value:'{"name":"bluez_input.AC_90_85_00_00_00.0"}' type:'Spa:String:JSON'
update: id:0 key:'default.video.source' value:'{"name":"v4l2_input.pci-0000_00_14.0-usb-0_1_1.0"}' type:'Spa:String:JSON'
"#;

#[test]
fn the_node_a_default_stands_for_is_read_from_its_key() {
    assert_eq!(
        standing_for(PRINTED, "default.audio.source").as_deref(),
        Some("bluez_input.AC_90_85_00_00_00.0")
    );
}

#[test]
fn the_live_default_is_read_not_the_configured_one() {
    assert_eq!(
        standing_for(PRINTED, "default.audio.sink").as_deref(),
        Some("alsa_output.pci-0000_00_1f.3.analog-stereo")
    );
}

#[test]
fn another_key_or_an_output_that_does_not_say_gives_nothing() {
    assert_eq!(standing_for(PRINTED, "default.audio.monitor"), None);
    assert_eq!(standing_for("", "default.audio.source"), None);
    assert_eq!(
        standing_for(
            "update: id:0 key:'default.audio.source' value:'not json' type:'Spa:String:JSON'",
            "default.audio.source"
        ),
        None
    );
    assert_eq!(
        standing_for(
            "update: id:0 key:'default.audio.source' value:'{}' type:'Spa:String:JSON'",
            "default.audio.source"
        ),
        None
    );
}
