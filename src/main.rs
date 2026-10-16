// A console must not appear behind the window on Windows. Debug builds keep
// it, so `println!` still works while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use camviewer::app;
use gpui_kit::*;

fn main() {
    // Answer before a window opens, so scripts can ask without a display.
    if std::env::args()
        .skip(1)
        .any(|argument| argument == "--version" || argument == "-V")
    {
        println!("camviewer {}", camviewer::update::current_version());
        return;
    }
    gpui_kit::application()
        // `AllAssets` embeds the complete Lucide icon set. The default `Assets`
        // carries only a small subset, which misses icons such as `Camera`.
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            // Registers the component library. Must run before anything else.
            gpui_kit::init(cx);

            // The sheet's own faces, before any window is opened.
            camviewer::fonts::register(cx);

            let bounds = Bounds::centered(None, size(px(1160.0), px(700.0)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(640.0), px(440.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("CamViewer".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| app::CamViewer::new(window, cx)),
            )
            .expect("failed to open the main window");
        });
}
