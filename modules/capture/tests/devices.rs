//! What a desktop's default device stands for, read from PipeWire's metadata as
//! `pw-metadata` prints it.

use summit_capture::standing_for;

const PRINTED: &str = r#"Found "default" metadata 40
update: id:0 key:'default.audio.source' value:'{"name":"bluez_input.AC_90_85_00_00_00.0"}' type:'Spa:String:JSON'
"#;

#[test]
fn the_node_a_default_stands_for_is_read_from_its_key() {
    assert_eq!(
        standing_for(PRINTED, "default.audio.source").as_deref(),
        Some("bluez_input.AC_90_85_00_00_00.0")
    );
}

#[test]
fn another_key_or_an_output_that_does_not_say_gives_nothing() {
    assert_eq!(standing_for(PRINTED, "default.audio.sink"), None);
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
