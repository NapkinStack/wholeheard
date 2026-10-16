//! What the shell does that needs no screen: it starts the steps' programs, reads what they
//! say as their contracts describe it, and turns that into what the user is told. The tray,
//! the shortcut and the window are built on this, and hold none of it.

pub mod model;
pub mod recording;
pub mod settings;
pub mod transcription;
pub mod words;
