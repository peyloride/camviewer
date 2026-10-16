//! CamViewer: a lean RTSP camera viewer built on GPUI.
//!
//! The crate is split so the UI can be tested without a display:
//!
//! - [`stream`] decodes RTSP to BGRA frames on a background thread.
//! - [`audio`] plays the camera audio track through `rodio`.
//! - [`config`] persists the camera list as JSON.
//! - [`update`] checks GitHub releases and swaps in a newer build.
//! - [`fonts`] carries the two type faces the sheet is set in.
//! - [`app`] holds the camera wall view.

pub mod app;
pub mod audio;
pub mod config;
pub mod fonts;
pub mod stream;
pub mod update;
