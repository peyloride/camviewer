//! The sheet's own type faces.
//!
//! The app ships to three platforms, so its voice cannot be whatever face a
//! machine happens to have installed: the drawing-office lettering is part of
//! the world, and a fallback is a different artifact. Both faces are under the
//! SIL Open Font Licence, and their licences ship beside them in
//! `assets/fonts`.

use std::borrow::Cow;

use gpui_kit::gpui::App;

/// Registers the faces the sheet is set in.
///
/// A machine whose text system refuses them is not a reason to fail: the sheet
/// then falls back to the platform's own faces and reads as a plainer sheet
/// rather than not at all.
pub fn register(cx: &App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Archivo-Variable.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexMono-SemiBold.ttf")),
    ];

    let _ = cx.text_system().add_fonts(fonts);
}

/// The face the sheet's own voice is set in.
pub const DISPLAY: &str = "Archivo";

/// The face the sheet's numbers and measurements are set in.
pub const NUMBERS: &str = "IBM Plex Mono";
