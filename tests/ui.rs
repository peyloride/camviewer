//! UI integration tests: a real view in a headless window, driven by clicks.
//!
//! GPUI's harness is switched on through the crates in `Cargo.toml` rather
//! than through `gpui-kit/test-support`, which does not resolve. See
//! `AGENTS.md` for that story. `TestWindowExt` lives behind the gpui-kit
//! feature, so the click helper below is the small part of it this file needs.

use camviewer::app::CamViewer;
use camviewer::config::CameraConfig;
use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, ElementId, Entity, InputEvent as _, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};

const WINDOW: (f32, f32) = (900.0, 640.0);

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            Default::default(),
            size(px(WINDOW.0), px(WINDOW.1)),
        ))),
        ..Default::default()
    }
}

/// Draws one frame.
///
/// `render_frame` lives behind the gpui-kit feature that does not resolve, so
/// this is the two calls it makes.
fn render_frame(window: &mut Window, cx: &mut App) {
    window.refresh();
    window.draw(cx).clear(cx);
}

/// Clicks the middle of an observed element.
///
/// A pressed and a released pointer event at the same place, which is what a
/// mouse does. The element has to have been drawn at least once first.
fn click(window: &mut Window, id: impl Into<ElementId>, cx: &mut App) {
    let id = id.into();
    render_frame(window, cx);

    let element = gpui_base::test_support::find(window, &[], &id)
        .unwrap_or_else(|| panic!("nothing on screen has the id {id:?}"));
    let middle = element.bounds().center();

    window.dispatch_event(
        MouseMoveEvent {
            position: middle,
            pressed_button: None,
            modifiers: Default::default(),
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position: middle,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    render_frame(window, cx);
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position: middle,
            click_count: 1,
            modifiers: Default::default(),
        }
        .to_platform_input(),
        cx,
    );
    render_frame(window, cx);
}

/// Opens the app in a headless window and hands it to the test.
fn open(
    cx: &mut TestAppContext,
    cameras: Vec<CameraConfig>,
) -> (AnyWindowHandle, Entity<CamViewer>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        gpui_kit::open_window(options(), cx, |window, cx| {
            cx.new(|cx| CamViewer::with_cameras(window, cx, cameras))
        })
        .expect("open the window")
    })
}

fn camera(name: &str) -> CameraConfig {
    CameraConfig {
        name: name.to_string(),
        // Nothing is listening, so the feed reports an error instead of
        // holding a camera. The tile still draws, which is what these tests
        // are about.
        url: "rtsp://127.0.0.1:1/nothing".to_string(),
        url_sd: None,
    }
}

#[gpui::test]
fn an_empty_wall_offers_to_add_a_camera(cx: &mut TestAppContext) {
    let (window_handle, view) = open(cx, Vec::new());

    cx.update_window(window_handle, |_, window, cx| {
        render_frame(window, cx);
        // The toolbar is there, and the wall has nothing on it.
        assert!(gpui_base::test_support::find(window, &[], &"add-camera".into()).is_some());
        assert_eq!(view.read(cx).camera_count(), 0);
        assert!(!view.read(cx).add_dialog_open());
    })
    .unwrap();
}

#[gpui::test]
fn clicking_add_camera_opens_the_dialog_and_cancel_closes_it(cx: &mut TestAppContext) {
    let (window_handle, view) = open(cx, Vec::new());

    cx.update_window(window_handle, |_, window, cx| {
        render_frame(window, cx);
        assert!(!view.read(cx).add_dialog_open());

        click(window, "add-camera", cx);
        assert!(
            view.read(cx).add_dialog_open(),
            "the button did not open it"
        );

        // The dialog's own controls are on screen.
        for id in [
            "name-input",
            "hd-input",
            "sd-input",
            "confirm-add",
            "cancel-add",
        ] {
            assert!(
                gpui_base::test_support::find(window, &[], &id.into()).is_some(),
                "{id} is missing from the dialog"
            );
        }

        click(window, "cancel-add", cx);
        assert!(!view.read(cx).add_dialog_open(), "cancel did not close it");
    })
    .unwrap();
}

#[gpui::test]
fn clicking_a_tile_enlarges_it_and_the_picture_returns_to_the_wall(cx: &mut TestAppContext) {
    let (window_handle, view) = open(cx, vec![camera("Front door"), camera("Back yard")]);

    cx.update_window(window_handle, |_, window, cx| {
        render_frame(window, cx);
        assert_eq!(view.read(cx).camera_count(), 2);
        assert_eq!(
            view.read(cx).enlarged_camera(),
            None,
            "the wall is the default view"
        );

        click(window, ("tile", 1usize), cx);
        assert_eq!(
            view.read(cx).enlarged_camera(),
            Some("Front door"),
            "clicking a tile did not enlarge its camera"
        );

        // The enlarged picture fills most of the window.
        let picture = gpui_base::test_support::find(window, &[], &("enlarged", 1usize).into())
            .expect("the enlarged picture");
        assert!(
            picture.bounds().size.width > px(WINDOW.0 * 0.8),
            "the enlarged picture is {} wide, expected most of {}",
            picture.bounds().size.width,
            WINDOW.0
        );

        click(window, ("enlarged", 1usize), cx);
        assert_eq!(
            view.read(cx).enlarged_camera(),
            None,
            "clicking the picture did not return to the wall"
        );
    })
    .unwrap();
}

#[gpui::test]
fn clicking_the_back_button_returns_to_the_wall(cx: &mut TestAppContext) {
    let (window_handle, view) = open(cx, vec![camera("Front door")]);

    cx.update_window(window_handle, |_, window, cx| {
        click(window, ("tile", 1usize), cx);
        assert_eq!(view.read(cx).enlarged_camera(), Some("Front door"));

        click(window, "show-all", cx);
        assert_eq!(view.read(cx).enlarged_camera(), None);
    })
    .unwrap();
}

#[gpui::test]
fn the_speaker_toggles_sound_one_camera_at_a_time(cx: &mut TestAppContext) {
    let (window_handle, view) = open(cx, vec![camera("Front door"), camera("Back yard")]);

    cx.update_window(window_handle, |_, window, cx| {
        render_frame(window, cx);
        assert_eq!(
            view.read(cx).is_muted("Front door"),
            Some(true),
            "cameras start muted"
        );

        click(window, ("mute", 1usize), cx);
        assert_eq!(view.read(cx).is_muted("Front door"), Some(false));

        // Unmuting a second camera takes the sound away from the first.
        click(window, ("mute", 2usize), cx);
        assert_eq!(view.read(cx).is_muted("Back yard"), Some(false));
        assert_eq!(view.read(cx).is_muted("Front door"), Some(true));

        // And the same button mutes it again.
        click(window, ("mute", 2usize), cx);
        assert_eq!(view.read(cx).is_muted("Back yard"), Some(true));
    })
    .unwrap();
}

#[gpui::test]
fn clicking_the_bin_removes_a_camera(cx: &mut TestAppContext) {
    let (window_handle, view) = open(cx, vec![camera("Front door"), camera("Back yard")]);

    cx.update_window(window_handle, |_, window, cx| {
        render_frame(window, cx);
        assert_eq!(view.read(cx).camera_count(), 2);

        click(window, ("remove", 1usize), cx);
        assert_eq!(view.read(cx).camera_count(), 1);
        assert!(view.read(cx).is_muted("Front door").is_none());
    })
    .unwrap();
}
