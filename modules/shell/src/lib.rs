//! What the shell does that needs no screen: it starts the steps' programs, reads what they
//! say as their contracts describe it, and turns that into what the user is told. The tray,
//! the shortcut and the window are built on this, and hold none of it.

pub mod desktop;
pub mod keys;
pub mod log;
pub mod meeting;
pub mod microphone;
pub mod migrate;
pub mod minutes;
pub mod mix;
pub mod model;
pub mod player;
pub mod providers;
pub mod recording;
pub mod sessions;
pub mod settings;
pub mod summarising;
pub mod transcription;
pub mod words;
