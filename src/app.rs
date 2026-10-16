//! The camera wall: toolbar, grid, enlarged view and the add dialog.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    text::TextView,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::audio::{AudioEngine, AudioHandle, PcmPipe};
use crate::config::{self, CameraConfig, WhatsNew};
use crate::stream::{Slot, Status, VideoStream};
use crate::update::{self, Release};

/// How often the UI looks for new frames.
const POLL_INTERVAL: Duration = Duration::from_millis(33);
/// Space between tiles, and between the wall and the window edge.
const GAP: f32 = 8.0;
/// Height of the toolbar.
const TOOLBAR_H: f32 = 48.0;
/// Corner rounding on a tile.
const TILE_RADIUS: f32 = 10.0;
/// Height of the enlarged view's header bar.
const HEADER_H: f32 = 34.0;
/// Height of the label bar along the bottom of a tile.
const BAR_H: f32 = 26.0;
/// Ink on top of video. Video is unpredictable, so this never follows the theme.
const ON_VIDEO: u32 = 0xffffffff;
/// A light wash for hovering something that sits on video.
const ON_VIDEO_HOVER: u32 = 0xffffff2e;

/// A compact chip-shaped control for use on top of video.
fn chip_button(
    id: (&'static str, u64),
    icon: IconName,
    label: Option<&'static str>,
    tooltip: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    h_flex()
        .id(id)
        .gap_1()
        .p_1()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .text_color(rgb(ON_VIDEO))
        .hover(|style| style.bg(rgba(ON_VIDEO_HOVER)))
        .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
        // Keep the click from reaching the video behind, which would navigate.
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .child(Icon::new(icon).xsmall().text_color(rgb(ON_VIDEO)))
        .when_some(label, |this, label| this.child(label))
        .into_any_element()
}

/// The squarest grid that holds every camera: one fills the window, two sit
/// side by side, three and four make a two-by-two block, five to nine make a
/// three-by-three block, and so on.
fn grid_columns(count: usize) -> usize {
    (count as f32).sqrt().ceil() as usize
}

/// The size of one cell in a `columns`-wide grid inside `avail_w` by `avail_h`.
///
/// Tiles fill their cell, so the wall covers the whole window. A stream that
/// is not the shape of its cell is letterboxed inside it.
fn tile_size(columns: usize, count: usize, avail_w: f32, avail_h: f32) -> (f32, f32) {
    let columns = columns.max(1);
    let rows = count.div_ceil(columns).max(1);
    let cell_w = (avail_w - (columns as f32 - 1.0) * GAP) / columns as f32;
    let cell_h = (avail_h - (rows as f32 - 1.0) * GAP) / rows as f32;

    (cell_w.max(1.0), cell_h.max(1.0))
}

/// A video surface of exactly this size.
///
/// The size must be given in pixels. GPUI's image element derives an
/// `aspect-ratio` from the picture when it has none, and that fights a
/// percentage height: the element grows taller than its parent and the parent
/// clips it. A definite size avoids the fight, and `Contain` then letterboxes
/// the picture inside it.
fn video(frame: Arc<RenderImage>, width: f32, height: f32) -> AnyElement {
    img(frame)
        .w(px(width))
        .h(px(height))
        // GPUI gives an image element an `aspect-ratio` from the picture when
        // it has none. Left alone, that ratio fights the definite size above
        // and the element overflows. State the element's own shape instead.
        .aspect_ratio(width / height)
        .object_fit(ObjectFit::Contain)
        .into_any_element()
}

/// The box the enlarged camera gets: everything under the header bar.
fn enlarged_size(avail_w: f32, avail_h: f32) -> (f32, f32) {
    (avail_w.max(1.0), (avail_h - HEADER_H - GAP).max(1.0))
}

/// What the app has to say about a newer build.
#[derive(Clone, Debug)]
enum UpdateState {
    /// Nothing to say, or the user dismissed the offer.
    Idle,
    Available(Release),
    Downloading(Release),
    Failed(String),
}

/// One decoding pipeline: one `ffmpeg` child, one thread, the newest frame.
struct Feed {
    /// Written by the decoder thread, read by the UI timer.
    slot: Arc<Mutex<Slot>>,
    /// The `seq` value already copied into the fields below.
    seen_seq: u64,
    frame: Option<Arc<RenderImage>>,
    status: Status,
    resolution: Option<(u32, u32)>,
    /// Dropping the stream stops the decoder thread.
    stream: VideoStream,
}

impl Feed {
    fn start(url: String) -> Self {
        let stream = VideoStream::start(url);
        let slot = stream.slot();
        Self {
            slot,
            seen_seq: 0,
            frame: None,
            status: Status::Connecting,
            resolution: None,
            stream,
        }
    }

    /// Copies a new frame or status out of the decoder. Returns true on change.
    fn poll(&mut self, retired: &mut Vec<Arc<RenderImage>>) -> bool {
        let Ok(slot) = self.slot.lock() else {
            return false;
        };
        if slot.seq == self.seen_seq {
            return false;
        }

        self.seen_seq = slot.seq;
        self.status = slot.status.clone();
        self.resolution = slot.resolution;

        if let Some(frame) = slot.frame.clone()
            && let Some(previous) = self.frame.replace(frame)
        {
            retired.push(previous);
        }
        true
    }

    /// Hands the current frame to the caller, to drop after this tick.
    fn clear(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        if let Some(frame) = self.frame.take() {
            retired.push(frame);
        }
    }

    /// This feed's sound, taken off its own video connection.
    fn audio(&self) -> PcmPipe {
        self.stream.audio()
    }
}

/// A camera the add dialog produced, once its fields were checked.
struct NewCamera {
    name: String,
    url_hd: String,
    url_sd: Option<String>,
}

/// Reads the add dialog's three fields.
///
/// The addresses are trimmed. An empty name becomes `Camera`, an empty
/// low-resolution address is dropped, and one that repeats the
/// high-resolution address is dropped as well: running the same stream twice
/// would cost a connection for nothing.
fn new_camera(name: &str, url_hd: &str, url_sd: &str) -> Result<NewCamera, &'static str> {
    let url_hd = url_hd.trim();
    if url_hd.is_empty() {
        return Err("Enter an RTSP URL for the high-resolution stream.");
    }

    let name = match name.trim() {
        "" => "Camera",
        name => name,
    };

    let url_sd = url_sd.trim();
    let url_sd = (!url_sd.is_empty() && url_sd != url_hd).then(|| url_sd.to_string());

    Ok(NewCamera {
        name: name.to_string(),
        url_hd: url_hd.to_string(),
        url_sd,
    })
}

/// Draw the full-resolution feed only when the camera is enlarged and that
/// feed is actually running. A camera left at its low-resolution picture is
/// better than a black tile.
fn draw_solo(enlarged: bool, solo_running: bool) -> bool {
    enlarged && solo_running
}

struct Camera {
    id: u64,
    name: SharedString,
    /// The full-resolution address, as configured.
    url_hd: String,
    /// The low-resolution address, as configured.
    url_sd: Option<String>,
    /// What the wall draws, and what covers for the full-resolution feed when
    /// that one cannot run. Started with the camera.
    wall: Feed,
    /// The full-resolution feed. Started the first time this camera is
    /// enlarged and then left running, so moving between the views never
    /// reconnects. `None` until then.
    solo: Option<Feed>,
    muted: bool,
    /// Present only while this camera plays sound.
    audio: Option<AudioHandle>,
    /// Lets the keyboard reach the tile.
    focus: FocusHandle,
}

impl Camera {
    /// The feed to draw right now.
    fn feed(&self, enlarged: bool) -> &Feed {
        match &self.solo {
            Some(solo) if draw_solo(enlarged, matches!(solo.status, Status::Live)) => solo,
            _ => &self.wall,
        }
    }

    /// Sound comes from whichever feed is live, so unmuting works even when
    /// the camera refused the full-resolution connection.
    fn audio(&self) -> PcmPipe {
        match &self.solo {
            Some(solo) if matches!(solo.status, Status::Live) => solo.audio(),
            _ => self.wall.audio(),
        }
    }

    /// Starts the full-resolution feed if this camera has not used it yet.
    fn warm_solo(&mut self) {
        if self.url_sd.is_some() && self.solo.is_none() {
            self.solo = Some(Feed::start(self.url_hd.clone()));
        }
    }

    fn poll(&mut self, retired: &mut Vec<Arc<RenderImage>>) -> bool {
        let mut changed = self.wall.poll(retired);
        if let Some(solo) = &mut self.solo {
            changed |= solo.poll(retired);
        }
        changed
    }

    fn clear(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        self.wall.clear(retired);
        if let Some(solo) = &mut self.solo {
            solo.clear(retired);
        }
    }
}

pub struct CamViewer {
    cameras: Vec<Camera>,
    next_id: u64,
    /// The camera shown alone, if any.
    enlarged: Option<u64>,
    show_add: bool,
    name_input: Entity<InputState>,
    hd_input: Entity<InputState>,
    sd_input: Entity<InputState>,
    add_error: Option<SharedString>,
    /// Where the camera list is saved. `None` in a test, which keeps the
    /// list in memory and never touches the user's file.
    config_path: Option<PathBuf>,
    update: UpdateState,
    /// Notes from the build that just replaced the old one, shown once.
    whats_new: Option<WhatsNew>,
    audio: Option<AudioEngine>,
    /// Frames replaced this tick. Dropped on the next tick, once the GPU has
    /// finished with them.
    retired: Vec<Arc<RenderImage>>,
    _subscriptions: Vec<Subscription>,
    _tick: Task<()>,
}

impl CamViewer {
    /// The view in normal use: the saved cameras, and a look for a newer
    /// release.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self::build(window, cx, config::load(), Some(config::path()));
        this.check_for_update(cx);
        this
    }

    /// Builds the view over an explicit camera list, reading and writing no
    /// files and asking nobody for an update.
    pub fn with_cameras(
        window: &mut Window,
        cx: &mut Context<Self>,
        cameras: Vec<CameraConfig>,
    ) -> Self {
        Self::build(window, cx, cameras, None)
    }

    /// A view for tests: no cameras, no file of its own, no network.
    pub fn for_test(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(window, cx, Vec::new(), None)
    }

    /// `config_path` is where the camera list is kept. `None` keeps it in
    /// memory only, which is what a test wants.
    fn build(
        window: &mut Window,
        cx: &mut Context<Self>,
        cameras: Vec<CameraConfig>,
        config_path: Option<PathBuf>,
    ) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Front door"));
        let hd_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("rtsp://user:password@192.168.1.10:554/stream1")
        });
        let sd_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("rtsp://user:password@192.168.1.10:554/stream2")
        });

        // Follow the operating system appearance instead of the light default.
        // To force one theme, replace this with `Theme::change(ThemeMode::Dark, None, cx)`.
        let mut subscriptions = vec![window.observe_window_appearance(|window, cx| {
            let mode = match window.appearance() {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
                _ => ThemeMode::Light,
            };
            Theme::change(mode, Some(window), cx);
        })];

        for input in [&name_input, &hd_input, &sd_input] {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                |this, _input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.submit_new_camera(window, cx);
                    }
                },
            ));
        }

        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        });

        let mut this = Self {
            cameras: Vec::new(),
            next_id: 1,
            enlarged: None,
            show_add: false,
            name_input,
            hd_input,
            sd_input,
            add_error: None,
            config_path: config_path.clone(),
            update: UpdateState::Idle,
            whats_new: config_path.as_ref().and_then(|_| config::load_whats_new()),
            audio: None,
            retired: Vec::new(),
            _subscriptions: subscriptions,
            _tick: tick,
        };

        for saved in cameras {
            this.push_camera(saved.name, saved.url, saved.url_sd, cx);
        }

        this
    }

    // --------------------------------------------------------------- readers

    /// Number of cameras on the wall.
    pub fn camera_count(&self) -> usize {
        self.cameras.len()
    }

    /// The name of the camera shown alone, if any.
    pub fn enlarged_camera(&self) -> Option<&str> {
        let id = self.enlarged?;
        self.cameras
            .iter()
            .find(|camera| camera.id == id)
            .map(|camera| camera.name.as_ref())
    }

    /// Whether the camera with this name is muted.
    pub fn is_muted(&self, name: &str) -> Option<bool> {
        self.cameras
            .iter()
            .find(|camera| camera.name.as_ref() == name)
            .map(|camera| camera.muted)
    }

    /// Whether the add-camera dialog is open.
    pub fn add_dialog_open(&self) -> bool {
        self.show_add
    }

    // ---------------------------------------------------------------- state

    /// Copies new frames and status changes out of the decoder slots.
    fn poll(&mut self, cx: &mut Context<Self>) {
        // Drop last tick's frames now: the previous scene is already presented.
        for image in std::mem::take(&mut self.retired) {
            cx.drop_image(image, None);
        }

        let mut retired = Vec::new();
        let mut changed = false;

        for camera in &mut self.cameras {
            changed |= camera.poll(&mut retired);
        }

        self.retired = retired;

        if changed {
            cx.notify();
        }
    }

    fn push_camera(
        &mut self,
        name: String,
        url_hd: String,
        url_sd: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_id;
        self.next_id += 1;

        // The wall runs the low-resolution stream when there is one. The
        // full-resolution feed starts only when this camera is first enlarged.
        let wall_url = url_sd.clone().unwrap_or_else(|| url_hd.clone());

        self.cameras.push(Camera {
            id,
            name: name.into(),
            url_hd,
            url_sd,
            wall: Feed::start(wall_url),
            solo: None,
            muted: true,
            audio: None,
            focus: cx.focus_handle(),
        });

        self.persist();
        cx.notify();
    }

    fn remove_camera(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.cameras.iter().position(|camera| camera.id == id) else {
            return;
        };

        let mut camera = self.cameras.remove(index);
        camera.clear(&mut self.retired);
        if self.enlarged == Some(id) {
            self.enlarged = None;
        }

        self.persist();
        cx.notify();
    }

    /// Shows one camera alone, or returns to the wall with `None`.
    ///
    /// The full-resolution feed starts here, on the first look, and then
    /// stays. After that this only changes which feed is drawn, so moving
    /// between the wall and the enlarged view is instant.
    fn set_enlarged(&mut self, id: Option<u64>, cx: &mut Context<Self>) {
        if let Some(id) = id
            && let Some(camera) = self.cameras.iter_mut().find(|camera| camera.id == id)
        {
            camera.warm_solo();
        }
        self.enlarged = id;
        cx.notify();
    }

    /// Turns sound on or off. Only one camera plays sound at a time.
    fn toggle_mute(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.cameras.iter().position(|camera| camera.id == id) else {
            return;
        };
        let mute_now = !self.cameras[index].muted;

        if !mute_now {
            for camera in &mut self.cameras {
                if camera.id != id {
                    camera.muted = true;
                    camera.audio = None;
                }
            }
        }

        self.cameras[index].muted = mute_now;

        // The sound comes off the camera's own video connection, so it needs
        // no session of its own and never reconnects when the view changes.
        let pipe = self.cameras[index].audio();
        let audio = if mute_now {
            None
        } else {
            if self.audio.is_none() {
                self.audio = AudioEngine::new();
            }
            self.audio.as_ref().map(|engine| engine.play(&pipe))
        };
        self.cameras[index].audio = audio;

        cx.notify();
    }

    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_add = true;
        self.add_error = None;
        let handle = self.name_input.read(cx).focus_handle(cx).clone();
        window.focus(&handle, cx);
        cx.notify();
    }

    fn close_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_add = false;
        self.add_error = None;
        for input in [
            self.name_input.clone(),
            self.hd_input.clone(),
            self.sd_input.clone(),
        ] {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
        cx.notify();
    }

    fn submit_new_camera(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value();
        let url_hd = self.hd_input.read(cx).value();
        let url_sd = self.sd_input.read(cx).value();

        match new_camera(&name, &url_hd, &url_sd) {
            Ok(camera) => {
                self.push_camera(camera.name, camera.url_hd, camera.url_sd, cx);
                self.close_add(window, cx);
            }
            Err(message) => {
                self.add_error = Some(message.into());
                let handle = self.hd_input.read(cx).focus_handle(cx).clone();
                window.focus(&handle, cx);
                cx.notify();
            }
        }
    }

    // --------------------------------------------------------------- update

    /// Asks GitHub, off the UI thread, whether a newer build exists. A failure
    /// stays quiet: a missing network must not nag.
    fn check_for_update(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let current = update::current_version().to_string();
            let found = cx
                .background_spawn(async move { update::check(&current) })
                .await;

            if let Ok(Some(release)) = found {
                this.update(cx, |this, cx| {
                    this.update = UpdateState::Available(release);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Downloads the new build, swaps it in, and restarts into it.
    fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(release) = self.update.clone() else {
            return;
        };
        self.update = UpdateState::Downloading(release.clone());
        cx.notify();

        // Kept for the popup the new build shows on its first start.
        let version = release.version.clone();
        let notes = release.notes.clone();

        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_spawn(async move { update::install(&release) })
                .await;

            this.update(cx, |this, cx| match outcome {
                Ok(()) => {
                    // The new build shows these on its first start.
                    config::store_whats_new(&version, &notes);
                    cx.restart()
                }
                Err(message) => {
                    this.update = UpdateState::Failed(message);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn persist(&self) {
        let cameras: Vec<CameraConfig> = self
            .cameras
            .iter()
            .map(|camera| CameraConfig {
                name: camera.name.to_string(),
                url: camera.url_hd.clone(),
                url_sd: camera.url_sd.clone(),
            })
            .collect();
        if let Some(path) = &self.config_path {
            config::save_to(path, &cameras);
        }
    }

    // --------------------------------------------------------------- render

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> Div {
        let surface = cx.theme().title_bar;
        let border = cx.theme().title_bar_border;
        let muted = cx.theme().muted_foreground;
        let count = self.cameras.len();

        h_flex()
            .w_full()
            .h(px(TOOLBAR_H))
            .px_3()
            .justify_between()
            .items_center()
            .bg(surface)
            .border_b_1()
            .border_color(border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::Camera).small().text_color(muted))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("CamViewer"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().text_xs().text_color(muted).child(match count {
                        1 => "1 camera".to_string(),
                        _ => format!("{count} cameras"),
                    }))
                    .child(
                        Button::new("add-camera")
                            .small()
                            .primary()
                            .icon(IconName::Plus)
                            .label("Add camera")
                            .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx))),
                    ),
            )
    }

    /// A strip under the toolbar while a newer build is on offer.
    fn render_update_banner(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let accent = cx.theme().accent;
        let foreground = cx.theme().accent_foreground;
        let danger = cx.theme().danger;

        let (icon, message, actions): (IconName, SharedString, Option<AnyElement>) = match &self
            .update
        {
            UpdateState::Idle => return None,
            UpdateState::Available(release) => (
                IconName::Download,
                format!("CamViewer {} is ready to install.", release.version).into(),
                Some(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("update-later")
                                .small()
                                .ghost()
                                .label("Later")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.update = UpdateState::Idle;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("update-install")
                                .small()
                                .primary()
                                .label("Install and restart")
                                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
                        )
                        .into_any_element(),
                ),
            ),
            UpdateState::Downloading(release) => (
                IconName::LoaderCircle,
                format!("Downloading CamViewer {}…", release.version).into(),
                None,
            ),
            UpdateState::Failed(message) => (
                IconName::TriangleAlert,
                message.clone().into(),
                Some(
                    Button::new("update-dismiss")
                        .small()
                        .ghost()
                        .label("Dismiss")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.update = UpdateState::Idle;
                            cx.notify();
                        }))
                        .into_any_element(),
                ),
            ),
        };

        let tint = if matches!(self.update, UpdateState::Failed(_)) {
            danger
        } else {
            foreground
        };

        Some(
            h_flex()
                .w_full()
                .px_3()
                .py_2()
                .gap_2()
                .items_center()
                .bg(accent)
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Icon::new(icon).small().text_color(tint))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(tint)
                        .truncate()
                        .child(message),
                )
                .children(actions)
                .into_any_element(),
        )
    }

    fn render_grid(&self, window: &Window, cx: &Context<Self>) -> Div {
        if self.cameras.is_empty() {
            return self.render_empty(cx);
        }

        let viewport = window.viewport_size();
        let avail_w = f32::from(viewport.width) - 2.0 * GAP;
        let avail_h = f32::from(viewport.height) - TOOLBAR_H - 2.0 * GAP;
        let columns = grid_columns(self.cameras.len());
        let (tile_w, tile_h) = tile_size(columns, self.cameras.len(), avail_w, avail_h);

        let mut rows: Vec<AnyElement> = Vec::new();
        let mut slot = 0usize;

        for row in self.cameras.chunks(columns) {
            let mut cells: Vec<AnyElement> = row
                .iter()
                .map(|camera| self.render_tile(camera, tile_w, tile_h, cx))
                .collect();

            // Fill the short last row, so the wall has no holes.
            for _ in row.len()..columns {
                cells.push(self.render_add_slot(slot, tile_w, tile_h, cx));
                slot += 1;
            }

            rows.push(
                h_flex()
                    .gap(px(GAP))
                    .justify_center()
                    .children(cells)
                    .into_any_element(),
            );
        }

        v_flex()
            .size_full()
            .gap(px(GAP))
            .justify_center()
            .children(rows)
    }

    fn render_empty(&self, cx: &Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(Icon::new(IconName::VideoOff).large().text_color(muted))
            .child(
                v_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::MEDIUM)
                            .child("No cameras"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("Add an RTSP URL to start watching."),
                    ),
            )
    }

    fn render_tile(
        &self,
        camera: &Camera,
        width: f32,
        height: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = camera.id;
        let border = cx.theme().border;
        let primary = cx.theme().primary;
        let group = SharedString::from(format!("tile-{id}"));

        div()
            .id(("tile", id))
            .group(group.clone())
            .flex()
            .flex_col()
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .rounded(px(TILE_RADIUS))
            .border_1()
            .border_color(border)
            .bg(rgb(0x000000))
            .cursor_pointer()
            .track_focus(&camera.focus)
            .hover(move |style| style.border_color(primary))
            .on_click(cx.listener(move |this, _, _, cx| this.set_enlarged(Some(id), cx)))
            // The bar keeps its own row, so it never covers the picture.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    // The wall never shows an enlarged camera.
                    .child(self.render_surface(camera, false, width, height - BAR_H, cx)),
            )
            .child(
                h_flex()
                    .flex_none()
                    .w_full()
                    .h(px(BAR_H))
                    .px_2()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .bg(cx.theme().title_bar)
                    .child(
                        div()
                            .min_w_0()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(camera.name.clone()),
                    )
                    .child(self.render_controls(camera, group, cx)),
            )
            .into_any_element()
    }

    /// An empty cell in the wall. It invites the next camera instead of
    /// leaving a hole.
    fn render_add_slot(
        &self,
        index: usize,
        width: f32,
        height: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;

        v_flex()
            .id(("add-slot", index as u64))
            .items_center()
            .justify_center()
            .gap_2()
            .w(px(width))
            .h(px(height))
            .rounded(px(TILE_RADIUS))
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .hover(|style| style.bg(rgba(0xffffff0a)))
            .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx)))
            .child(Icon::new(IconName::Plus).small().text_color(muted))
            .child(div().text_xs().text_color(muted).child("Add camera"))
            .into_any_element()
    }

    /// The mute and remove controls, on a dark scrim so they read over video.
    /// Remove stays hidden until the tile is hovered, so it is never a slip.
    fn render_controls(
        &self,
        camera: &Camera,
        group: SharedString,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = camera.id;
        let (mute_icon, mute_tip) = if camera.muted {
            (IconName::VolumeX, "Unmute")
        } else {
            (IconName::Volume2, "Mute")
        };

        h_flex()
            .flex_none()
            .gap_1()
            // Keep control clicks away from the tile behind.
            .occlude()
            .child(chip_button(
                ("mute", id),
                mute_icon,
                None,
                mute_tip,
                cx.listener(move |this, _, _, cx| this.toggle_mute(id, cx)),
            ))
            .child(
                div()
                    .id(("remove-slot", id))
                    .opacity(0.0)
                    .group_hover(group, |style| style.opacity(1.0))
                    .child(chip_button(
                        ("remove", id),
                        IconName::Trash,
                        None,
                        "Remove camera",
                        cx.listener(move |this, _, _, cx| this.remove_camera(id, cx)),
                    )),
            )
            .into_any_element()
    }

    /// The mute and remove controls for the enlarged header. They sit on the
    /// app surface, so they wear theme colours like the button beside them.
    fn render_header_controls(&self, camera: &Camera, cx: &Context<Self>) -> AnyElement {
        let id = camera.id;
        let (mute_icon, mute_tip) = if camera.muted {
            (IconName::VolumeX, "Unmute")
        } else {
            (IconName::Volume2, "Mute")
        };

        h_flex()
            .gap_1()
            .child(
                Button::new(("header-mute", id))
                    .small()
                    .ghost()
                    .icon(mute_icon)
                    .tooltip(mute_tip)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_mute(id, cx))),
            )
            .child(
                Button::new(("header-remove", id))
                    .small()
                    .ghost()
                    .icon(IconName::Trash)
                    .tooltip("Remove camera")
                    .on_click(cx.listener(move |this, _, _, cx| this.remove_camera(id, cx))),
            )
            .into_any_element()
    }

    fn render_surface(
        &self,
        camera: &Camera,
        enlarged: bool,
        width: f32,
        height: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let feed = camera.feed(enlarged);

        if let Some(frame) = &feed.frame {
            return video(frame.clone(), width, height);
        }

        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;

        match &feed.status {
            Status::Error(message) => v_flex()
                .w(px(width))
                .h(px(height))
                .items_center()
                .justify_center()
                .gap_2()
                .px_4()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .small()
                        .text_color(danger),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(danger)
                        .text_center()
                        .child(message.clone()),
                )
                .into_any_element(),
            _ => v_flex()
                .w(px(width))
                .h(px(height))
                .items_center()
                .justify_center()
                .child(div().text_xs().text_color(muted).child("Connecting…"))
                .into_any_element(),
        }
    }

    fn render_enlarged(&self, id: u64, window: &Window, cx: &Context<Self>) -> Div {
        let Some(camera) = self.cameras.iter().find(|camera| camera.id == id) else {
            return div();
        };

        let border = cx.theme().border;
        let surface = cx.theme().title_bar;
        let muted = cx.theme().muted_foreground;
        let feed = camera.feed(true);
        let detail = match (&feed.status, feed.resolution) {
            (Status::Live, Some((width, height))) => format!("Live · {width}×{height}"),
            (Status::Live, None) => "Live".to_string(),
            (Status::Connecting, _) => "Connecting…".to_string(),
            (Status::Error(message), _) => message.clone(),
        };

        let viewport = window.viewport_size();
        let avail_w = f32::from(viewport.width) - 2.0 * GAP;
        let avail_h = f32::from(viewport.height) - TOOLBAR_H - 2.0 * GAP;
        let (video_w, video_h) = enlarged_size(avail_w, avail_h);

        v_flex()
            .size_full()
            .gap(px(GAP))
            .child(
                h_flex()
                    .w_full()
                    .h(px(HEADER_H))
                    .px_2()
                    .gap_3()
                    .items_center()
                    .rounded(px(TILE_RADIUS))
                    .bg(surface)
                    .child(
                        Button::new("show-all")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .tooltip("All cameras")
                            .on_click(cx.listener(|this, _, _, cx| this.set_enlarged(None, cx))),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(camera.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(muted)
                                    .truncate()
                                    .child(detail),
                            ),
                    )
                    .child(self.render_header_controls(camera, cx)),
            )
            .child(
                div()
                    .id(("enlarged", id))
                    .relative()
                    .w(px(video_w))
                    .h(px(video_h))
                    .overflow_hidden()
                    .rounded(px(TILE_RADIUS))
                    .border_1()
                    .border_color(border)
                    .bg(rgb(0x000000))
                    .cursor_pointer()
                    // Clicking the picture returns to the wall.
                    .on_click(cx.listener(|this, _, _, cx| this.set_enlarged(None, cx)))
                    .child(self.render_surface(camera, true, video_w, video_h, cx)),
            )
    }

    /// What changed in the build that was just installed. Shown once.
    fn render_whats_new(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let popover = cx.theme().popover;
        let muted = cx.theme().muted_foreground;
        let version = self
            .whats_new
            .as_ref()
            .map(|release| release.version.clone())
            .unwrap_or_default();
        let notes = self
            .whats_new
            .as_ref()
            .map(|release| release.notes.clone())
            .unwrap_or_default();

        let dismiss = cx.listener(|this, _, _, cx| {
            config::clear_whats_new();
            this.whats_new = None;
            cx.notify();
        });

        div()
            .absolute()
            .inset_0()
            .occlude()
            .id("whats-new-backdrop")
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000090))
            .on_click(cx.listener(|this, _, _, cx| {
                config::clear_whats_new();
                this.whats_new = None;
                cx.notify();
            }))
            .child(
                v_flex()
                    .id("whats-new-dialog")
                    .occlude()
                    .w(px(560.0))
                    .p_5()
                    .gap_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(border)
                    .bg(popover)
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("What's new in CamViewer {version}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child("Updated builds show this once."),
                            ),
                    )
                    .child(
                        div()
                            .id("whats-new-notes")
                            .max_h(px(340.0))
                            .overflow_y_scroll()
                            .child(TextView::markdown("whats-new-body", notes)),
                    )
                    .child(
                        h_flex().w_full().justify_end().child(
                            Button::new("whats-new-ok")
                                .primary()
                                .label("Got it")
                                .on_click(dismiss),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_add_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let popover = cx.theme().popover;
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;

        let field = |label: &'static str,
                     id: &'static str,
                     hint: Option<&'static str>,
                     input: &Entity<InputState>| {
            v_flex()
                .gap_1()
                .child(div().text_xs().text_color(muted).child(label))
                .child(Input::new(input).id(id).cleanable(true))
                .when_some(hint, |this, hint| {
                    this.child(div().text_xs().text_color(muted).child(hint))
                })
        };

        let mut dialog = v_flex()
            .id("add-camera-dialog")
            .occlude()
            .w(px(500.0))
            .p_5()
            .gap_4()
            .rounded_lg()
            .border_1()
            .border_color(border)
            .bg(popover)
            // A click inside the card must not reach the backdrop below.
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Add camera"),
            )
            .child(field("Name", "name-input", None, &self.name_input))
            .child(field(
                "High-resolution RTSP URL",
                "hd-input",
                None,
                &self.hd_input,
            ))
            .child(field(
                "Low-resolution RTSP URL — optional",
                "sd-input",
                Some("The wall shows this stream to save bandwidth. Enlarging a camera switches to the URL above."),
                &self.sd_input,
            ));

        if let Some(error) = &self.add_error {
            dialog = dialog.child(div().text_xs().text_color(danger).child(error.clone()));
        }

        dialog = dialog.child(
            h_flex()
                .w_full()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("cancel-add")
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, window, cx| this.close_add(window, cx))),
                )
                .child(Button::new("confirm-add").primary().label("Add").on_click(
                    cx.listener(|this, _, window, cx| this.submit_new_camera(window, cx)),
                )),
        );

        div()
            .absolute()
            .inset_0()
            .occlude()
            .id("add-camera-backdrop")
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000090))
            .on_click(cx.listener(|this, _, window, cx| this.close_add(window, cx)))
            .child(dialog)
            .into_any_element()
    }
}

impl Render for CamViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.enlarged {
            Some(id) => self.render_enlarged(id, window, cx),
            None => self.render_grid(window, cx),
        };

        let mut root = v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_toolbar(cx))
            .children(self.render_update_banner(cx))
            .child(div().flex_1().min_h_0().w_full().p(px(GAP)).child(body));

        if self.show_add {
            root = root.child(self.render_add_overlay(cx));
        }
        if self.whats_new.is_some() {
            root = root.child(self.render_whats_new(cx));
        }

        root
    }
}

#[cfg(test)]
mod tests {
    // Import by name: a glob would bring in GPUI's `test` macro and shadow
    // the built-in `#[test]` attribute.
    use super::{GAP, HEADER_H, draw_solo, enlarged_size, grid_columns, new_camera, tile_size};

    #[test]
    fn the_wall_uses_the_squarest_grid_that_fits() {
        assert_eq!(grid_columns(1), 1);
        assert_eq!(grid_columns(2), 2);
        assert_eq!(grid_columns(3), 2);
        assert_eq!(grid_columns(4), 2);
        assert_eq!(grid_columns(5), 3);
        assert_eq!(grid_columns(6), 3);
        assert_eq!(grid_columns(9), 3);
        assert_eq!(grid_columns(10), 4);
        assert_eq!(grid_columns(16), 4);
        assert_eq!(grid_columns(17), 5);
    }

    #[test]
    fn one_camera_fills_the_window() {
        let (w, h) = tile_size(1, 1, 1600.0, 900.0);
        assert!((w - 1600.0).abs() < 0.01, "{w}");
        assert!((h - 900.0).abs() < 0.01, "{h}");
    }

    #[test]
    fn two_cameras_split_the_width_in_half() {
        let (w, h) = tile_size(2, 2, 1600.0, 900.0);
        assert!((w - 796.0).abs() < 0.01, "{w}");
        assert!((h - 900.0).abs() < 0.01, "{h}");
        // Both tiles and the gap fill the width exactly.
        assert!((w * 2.0 + GAP - 1600.0).abs() < 0.01);
    }

    #[test]
    fn the_wall_covers_the_whole_area() {
        // Three cameras make a two-by-two block, so the second row is half full
        // but both rows still reach the bottom of the window.
        let (w, h) = tile_size(2, 3, 1600.0, 900.0);
        assert!((w * 2.0 + GAP - 1600.0).abs() < 0.01);
        assert!((h * 2.0 + GAP - 900.0).abs() < 0.01);
    }

    #[test]
    fn a_tiny_window_still_yields_a_drawable_tile() {
        let (w, h) = tile_size(3, 9, 1.0, 1.0);
        assert!(w >= 1.0 && h >= 1.0, "{w}x{h}");
    }

    #[test]
    fn the_enlarged_box_leaves_room_for_the_header_bar() {
        let (w, h) = enlarged_size(1600.0, 900.0);
        assert!((w - 1600.0).abs() < 0.01, "{w}");
        assert!((h - (900.0 - HEADER_H - GAP)).abs() < 0.01, "{h}");
    }

    #[test]
    fn a_camera_needs_a_high_resolution_address() {
        assert!(new_camera("Front door", "   ", "").is_err());
        assert!(new_camera("", "", "").is_err());
    }

    #[test]
    fn a_camera_without_a_name_is_called_camera() {
        let camera = new_camera("   ", "rtsp://cam/1", "").expect("valid");
        assert_eq!(camera.name, "Camera");
    }

    #[test]
    fn the_addresses_are_trimmed() {
        let camera = new_camera("  Front door  ", "  rtsp://cam/1  ", "  ").expect("valid");
        assert_eq!(camera.name, "Front door");
        assert_eq!(camera.url_hd, "rtsp://cam/1");
        assert_eq!(camera.url_sd, None);
    }

    #[test]
    fn a_low_resolution_address_that_repeats_the_high_resolution_one_is_dropped() {
        let camera = new_camera("Front door", "rtsp://cam/1", "rtsp://cam/1").expect("valid");
        assert_eq!(camera.url_sd, None);
    }

    #[test]
    fn a_different_low_resolution_address_is_kept() {
        let camera = new_camera("Front door", "rtsp://cam/1", "rtsp://cam/2").expect("valid");
        assert_eq!(camera.url_sd.as_deref(), Some("rtsp://cam/2"));
    }

    #[test]
    fn enlarging_draws_the_full_resolution_feed_once_it_runs() {
        assert!(draw_solo(true, true));
    }

    #[test]
    fn the_wall_never_draws_the_full_resolution_feed() {
        assert!(!draw_solo(false, true));
        assert!(!draw_solo(false, false));
    }

    #[test]
    fn an_enlarged_camera_falls_back_while_the_full_resolution_feed_is_down() {
        // The camera refused the second session, or it has not started yet.
        assert!(!draw_solo(true, false));
    }
}
