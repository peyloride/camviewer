//! The camera wall: toolbar, grid, enlarged view and the add dialog.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::TestSupportExt as _;
use gpui_kit::component::{
    ActiveTheme as _, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    text::TextView,
    tooltip::Tooltip,
    v_flex, window_paddings,
};
use gpui_kit::*;

use crate::audio::{AudioEngine, AudioHandle, PcmPipe};
use crate::config::{self, CameraConfig, WhatsNew};
use crate::stream::{Slot, Status, VideoStream};
use crate::update::{self, Release};

/// How often the UI looks for new frames.
const POLL_INTERVAL: Duration = Duration::from_millis(33);

// ------------------------------------------------------------------ sheet
//
// The sheet is one artifact: a rack elevation drawn on vellum. Its measurements
// are the drawing's, so they are fixed and the wall gives way around them.

/// Height of the title block across the top of the sheet.
const TITLE_H: f32 = 88.0;
/// Width of the numbered margin gutter down the left edge.
const MARGIN_W: f32 = 54.0;
/// Height of the dimension row above the wall.
const DIM_H: f32 = 26.0;
/// Height of the caption strip along the bottom of a unit.
const CAPTION_H: f32 = 42.0;
/// Space between units.
const GAP: f32 = 10.0;
/// Space between the sheet's edge and its furniture.
const PAD: f32 = 26.0;
/// The lightest rule on the sheet.
const HAIRLINE_W: f32 = 1.0;
/// The rule that carries weight: under the title block and around the wall.
const RULE_W: f32 = 2.0;
/// A unit narrower than this gives up its larger type and its secondary column,
/// so the name and the state keep their room.
const COMPACT_BELOW: f32 = 400.0;
/// A unit at least this wide carries its address and its controls in the open,
/// beside the name and the state.
///
/// Narrower, both wait: a truncated address is not an address, and a name cut in
/// half is a different camera. The name and the state are what the wall exists
/// for, so they are what stays.
const WIDE_ENOUGH: f32 = 560.0;
/// Below this width the title block gives up its first piece of furniture, and
/// below the second it gives up the legend as well.
const META_ABOVE: f32 = 780.0;
const LEGEND_ABOVE: f32 = 1040.0;

// The sheet's colours. A drawing sheet is one artifact, so they never follow the
// operating system's light or dark appearance.
/// The vellum the sheet is drawn on.
const VELLUM: u32 = 0xfcfcfb;
/// Graphite: every rule, every name, every number.
const GRAPHITE: u32 = 0x1a1c1e;
/// A pencil hairline, for the softest rules.
const HAIRLINE: u32 = 0xc9c7c4;
/// Construction furniture, ruled in the blue reproduction cannot see.
const NON_REPRO: u32 = 0xa8c6e8;
/// Claimed by the sheet for one job only: a unit that is not normal.
const FAULT: u32 = 0x1d4ed8;
/// A caption plate, and any surface that must sit above the vellum.
const PLATE: u32 = 0xffffff;
/// The picture field. Video is unpredictable, so this never follows the theme.
const PICTURE: u32 = 0x0b0d0e;
/// Secondary words: the host, the sheet meta, a quiet state.
const MUTED: u32 = 0x6b7075;

/// A quiet state printed inside a picture field, where the ground is near-black
/// and the caption's grey would not carry. The drafting blue is too dark to read
/// there, so a cause is printed in the sheet's pale non-repro blue instead, and
/// the drafting blue stays on the frame, the rule and the caption word.
const QUIET_ON_PICTURE: u32 = 0x9aa0a6;

/// Space between the title block and the top of the sheet's body.
const BODY_TOP: f32 = 14.0;
/// The strip a new release adds under the title block.
const STRIP_H: f32 = 42.0;

/// A stream that has been live but has produced no frame for this long is
/// reported as frozen rather than drawn as if it were live.
const FROZEN_AFTER: Duration = Duration::from_secs(3);

/// What the sheet says about one unit.
///
/// The state is printed, never implied. The wall's whole job is to be read at a
/// glance and never to look live when it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZoneState {
    Live,
    Connecting,
    Frozen,
    NoSignal,
}

impl ZoneState {
    /// The word printed in the unit's caption.
    fn word(self) -> &'static str {
        match self {
            Self::Live => "Live",
            Self::Connecting => "Connecting",
            Self::Frozen => "Frozen",
            Self::NoSignal => "No signal",
        }
    }

    /// Whether the sheet marks this unit with the fault colour. A unit carries
    /// the fault mark only when the sheet has something wrong to report.
    fn is_fault(self) -> bool {
        matches!(self, Self::Frozen | Self::NoSignal)
    }
}

/// The state to print for one camera.
///
/// `since_frame` is how long ago the decoder last published a frame. A stream
/// that reports itself live and then stops producing frames is frozen, not live:
/// that difference is the one thing this wall exists to tell.
fn zone_state(status: &Status, has_frame: bool, since_frame: Duration) -> ZoneState {
    match status {
        Status::Connecting => ZoneState::Connecting,
        Status::Error(_) => ZoneState::NoSignal,
        // Live, but nothing drawn yet, is still a connection in progress.
        Status::Live if !has_frame => ZoneState::Connecting,
        Status::Live if since_frame > FROZEN_AFTER => ZoneState::Frozen,
        Status::Live => ZoneState::Live,
    }
}

/// A short cause to print in a unit that is not live.
///
/// The decoder reports what ffmpeg said, which is written for someone reading a
/// log. The sheet prints the cause in the words a person would use, and keeps
/// ffmpeg's own words for the tooltip.
fn fault_reason(message: &str) -> &'static str {
    let message = message.to_ascii_lowercase();
    let has = |needle: &str| message.contains(needle);

    if has("cannot run ffmpeg") {
        "ffmpeg is not installed"
    } else if has("404") || has("not found") {
        "Address not found"
    } else if has("401") || has("unauthorized") {
        "Password refused"
    } else if has("403") || has("not acceptable") || has("operation not permitted") {
        "Session refused"
    } else if has("connection refused") {
        "Connection refused"
    } else if has("no route to host") || has("unreachable") {
        "Host unreachable"
    } else if has("timed out") || has("timeout") {
        "Timed out"
    } else if has("name or service not known") || has("resolve") {
        "Address not known"
    } else if has("invalid data") || has("could not find codec") {
        "Not a video stream"
    } else {
        "Stream ended"
    }
}

/// `mm:ss`, so a camera that has been connecting for an hour cannot be mistaken
/// for one that is about to answer.
fn elapsed_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// The row reference printed in the margin, counted from the top the way a
/// drawing is read. The wall is two-dimensional, so a single numbered column
/// can name a row and never a unit: a number that implied otherwise would be a
/// false claim on a sheet whose whole job is telling the truth.
fn row_label(index: usize) -> String {
    format!("R{}", index + 1)
}

/// What the title block can afford to show at this width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SheetFurniture {
    /// The sheet's meta line under the brand.
    meta: bool,
    /// The legend that names the three state symbols.
    legend: bool,
}

fn sheet_furniture(width: f32) -> SheetFurniture {
    SheetFurniture {
        meta: width >= META_ABOVE,
        legend: width >= LEGEND_ABOVE,
    }
}

/// How a control is drawn on the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    /// An outline control on a plate: the sheet's default.
    Outline,
    /// A quieter outline, for a control that is secondary to the state beside
    /// it. The sound toggle sits next to the state and must not outshout it.
    Quiet,
    /// A filled control. One per view, for the action that view exists for.
    Filled,
}

/// A squared control drawn on the sheet.
///
/// The world supplies no controls of its own, so this is an ordinary button that
/// happens to be ruled and set in the sheet's voice. It never rounds a corner
/// and it is never blue: the fault colour is claimed by the one unit that has
/// something wrong with it.
fn sheet_button(
    id: (&'static str, u64),
    label: impl Into<SharedString>,
    tone: Tone,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let label: SharedString = label.into();

    // The control is the platform's own button, so the keyboard reaches it and
    // Enter or Space activates it without a pointer. Only its skin is the
    // sheet's, and the theme carries that.
    let mut button = Button::new(id).label(label.to_uppercase()).compact();
    button = match tone {
        // A ruled plate: a graphite rule with graphite type on the sheet's
        // paper. The fault blue is never spent on a control.
        Tone::Outline => button.primary().outline(),
        Tone::Filled => button.primary(),
        // A quiet control is a word with a hover. A control that only appears
        // under the pointer needs no box around it.
        Tone::Quiet => button.ghost(),
    };

    button
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .into_any_element()
}

/// A mono line of sheet furniture: measurement, meta, state.
///
/// The sheet's numbers are monospaced so they do not move when they change,
/// which is the whole reason a drawing is dimensioned in numbers.
fn mono_word(cx: &App, text: impl Into<SharedString>, size: f32, colour: u32) -> Div {
    div()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(size))
        .text_color(rgb(colour))
        .child(text.into())
}

/// The box one unit occupies.
///
/// The rule is the same weight whether or not the unit is in fault. A fault
/// changes the colour of a unit and never its size, because a wall that shifts
/// when something goes wrong is a wall you cannot read across a room.
#[derive(Clone, Copy, Debug)]
struct UnitBox {
    width: f32,
    height: f32,
}

impl UnitBox {
    /// The picture field and the caption plate, inside the unit's rule.
    fn picture(&self) -> (f32, f32) {
        (
            (self.width - 2.0 * RULE_W).max(1.0),
            (self.height - 2.0 * RULE_W - CAPTION_H).max(1.0),
        )
    }

    /// Whether the unit is small enough that its secondary column has to go.
    fn compact(&self) -> bool {
        self.width < COMPACT_BELOW
    }

    /// Whether the unit is wide enough to carry its address and its controls
    /// in the open. Narrower, both wait for the pointer.
    fn is_wide(&self) -> bool {
        self.width >= WIDE_ENOUGH
    }
}

/// The state symbol: filled for a unit that is live, half filled for a
/// connection in progress, hollow for a slot with nothing in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymbolFill {
    Full,
    Half,
    None,
}

fn symbol_for(state: ZoneState) -> SymbolFill {
    match state {
        ZoneState::Live | ZoneState::Frozen => SymbolFill::Full,
        ZoneState::Connecting => SymbolFill::Half,
        // Nothing is being drawn for this unit, so its symbol is empty too.
        ZoneState::NoSignal => SymbolFill::None,
    }
}

/// The words a unit prints for its state.
///
/// A connection that is still trying prints how long it has been trying, so a
/// camera that will never answer cannot be mistaken for one that is about to.
fn state_words(state: ZoneState, connecting_for: Duration) -> String {
    match state {
        ZoneState::Connecting => format!("CONNECTING {}", elapsed_label(connecting_for)),
        other => other.word().to_uppercase(),
    }
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

/// The box this view has inside the window surface, as an estimate.
///
/// `Window::viewport_size` is the whole surface, and a drawing frame keeps its
/// shadow and its border on the inside of it, so the surface is larger than the
/// space the view is drawn in. A wall measured against the surface hung out of
/// the window: its last row was cropped by exactly this much, and every unit was
/// squeezed to fit the row beside it.
///
/// This is what the first frame of a window uses. Every frame after it measures
/// the box the frame actually reports, which is exact.
fn inside_frame(viewport_w: f32, viewport_h: f32, pads: Edges<Pixels>) -> (f32, f32) {
    (
        (viewport_w - f32::from(pads.left) - f32::from(pads.right)).max(1.0),
        (viewport_h - f32::from(pads.top) - f32::from(pads.bottom)).max(1.0),
    )
}

/// The box the wall gets, once the sheet's own furniture has taken its share:
/// the title block and the dimension row across the top, the numbered gutter
/// down the left, and the page margin around everything.
///
/// `chrome_h` is whatever sits between the title block and the body, which is
/// the strip a new release adds and nothing the rest of the time.
fn wall_box(viewport_w: f32, viewport_h: f32, chrome_h: f32) -> (f32, f32) {
    (
        (viewport_w - 2.0 * PAD - MARGIN_W - GAP).max(1.0),
        (viewport_h - TITLE_H - chrome_h - BODY_TOP - DIM_H - PAD).max(1.0),
    )
}

/// Holds the sheet's appearance and its palette.
///
/// The window does not follow the operating system here. A rack elevation is
/// drawn on vellum in graphite, and inverting it would make it a different
/// artifact; a picture is also judged against the same ground at any hour.
/// Only the operating system's own window frame still follows the system.
fn paint_the_sheet(window: &mut Window, cx: &mut App) {
    Theme::change(ThemeMode::Light, Some(window), cx);
    Theme::update(cx, |theme| {
        theme.background = rgb(VELLUM).into();
        theme.foreground = rgb(GRAPHITE).into();
        theme.border = rgb(HAIRLINE).into();
        theme.muted = rgb(0xf0f0ee).into();
        theme.muted_foreground = rgb(MUTED).into();
        theme.popover = rgb(PLATE).into();
        theme.popover_foreground = rgb(GRAPHITE).into();
        theme.accent = rgb(0xf0f0ee).into();
        theme.accent_foreground = rgb(GRAPHITE).into();
        // Graphite is the one filled control on the sheet, so the fault blue
        // stays claimed by the unit that has something wrong with it.
        theme.primary = rgb(GRAPHITE).into();
        theme.primary_foreground = rgb(VELLUM).into();
        theme.primary_hover = rgb(0x33363a).into();
        theme.primary_active = rgb(0x000000).into();
        theme.input = rgb(PLATE).into();
        theme.caret = rgb(GRAPHITE).into();
        theme.ring = rgb(FAULT).into();
        theme.danger = rgb(FAULT).into();
        theme.danger_foreground = rgb(VELLUM).into();
        theme.title_bar = rgb(VELLUM).into();
        theme.title_bar_border = rgb(GRAPHITE).into();
        theme.font_family = crate::fonts::DISPLAY.into();
        theme.mono_font_family = crate::fonts::NUMBERS.into();
        // The sheet is drawn with rules, not with rounding or shadow.
        theme.radius = px(0.0);
        theme.radius_lg = px(0.0);
        theme.shadow = false;
        // A control on the sheet is a ruled plate, and its hover is a shade of
        // the paper rather than a wash of colour.
        theme.button = rgb(PLATE).into();
        theme.button_foreground = rgb(GRAPHITE).into();
        theme.button_hover = rgb(0xf0f0ee).into();
        theme.button_active = rgb(0xe4e4e1).into();
        theme.secondary = rgb(PLATE).into();
        // A ghost control is a quiet word on the paper, so it uses the sheet's
        // secondary ink rather than the full graphite.
        theme.secondary_foreground = rgb(MUTED).into();
        theme.button_primary = rgb(GRAPHITE).into();
        theme.button_primary_foreground = rgb(VELLUM).into();
        theme.button_primary_hover = rgb(0x33363a).into();
    });
}

/// How long the app waits before asking GitHub again after a check that
/// answered. A wall stays open for weeks, so it has to keep asking.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// A check that could not reach GitHub is retried much sooner. The first check
/// happens a second after the window opens, which is exactly when a machine's
/// network may still be settling.
const RETRY_AFTER: Duration = Duration::from_secs(10 * 60);

/// What one check of the releases page ended in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CheckOutcome {
    /// GitHub answered. Anything it had to say is on the sheet.
    Answered,
    /// GitHub could not be reached, or answered something unusable.
    Failed,
}

/// How long to wait before the next check.
fn wait_before_next_check(outcome: CheckOutcome) -> Duration {
    match outcome {
        CheckOutcome::Failed => RETRY_AFTER,
        CheckOutcome::Answered => CHECK_EVERY,
    }
}

/// Whether a release should be put on the sheet.
///
/// The same offer is never made twice in one run: answering `Later` means the
/// user has seen it. A newer build brings its own offer.
fn should_offer(version: &str, already_offered: Option<&str>) -> bool {
    already_offered != Some(version)
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
    /// When the decoder last published a frame.
    last_frame_at: Instant,
    /// When the current connection attempt started.
    connecting_since: Instant,
    /// The state already drawn, so that a change of state repaints the sheet
    /// even when no frame arrived to cause it.
    printed: ZoneState,
    /// Dropping the stream stops the decoder thread.
    stream: VideoStream,
}

impl Feed {
    fn start(url: String) -> Self {
        let stream = VideoStream::start(url);
        let slot = stream.slot();
        let now = Instant::now();
        Self {
            slot,
            seen_seq: 0,
            frame: None,
            status: Status::Connecting,
            resolution: None,
            last_frame_at: now,
            connecting_since: now,
            printed: ZoneState::Connecting,
            stream,
        }
    }

    /// Copies a new frame or status out of the decoder, then re-reads the state
    /// this feed prints. Returns true when anything the sheet draws changed.
    ///
    /// The state is read on every tick, not only when the decoder speaks. A
    /// stream that goes quiet has to stop claiming to be live, and nothing
    /// arrives from the decoder to say so.
    fn poll(&mut self, now: Instant, retired: &mut Vec<Arc<RenderImage>>) -> bool {
        let mut changed = false;

        if let Ok(slot) = self.slot.lock()
            && slot.seq != self.seen_seq
        {
            self.seen_seq = slot.seq;
            changed = true;

            if slot.status != self.status {
                self.status = slot.status.clone();
                if matches!(self.status, Status::Live) {
                    // A connection the decoder has just called live has not had
                    // time to draw anything. Measuring from here keeps it out of
                    // the frozen state while its first frames arrive.
                    self.last_frame_at = now;
                }
                if matches!(self.status, Status::Connecting) {
                    self.connecting_since = now;
                }
            }

            self.resolution = slot.resolution;

            if let Some(frame) = slot.frame.clone() {
                self.last_frame_at = now;
                if let Some(previous) = self.frame.replace(frame) {
                    retired.push(previous);
                }
            }
        }

        let printed = self.state(now);
        if printed != self.printed {
            self.printed = printed;
            changed = true;
        }

        changed
    }

    /// The state this feed prints right now.
    fn state(&self, now: Instant) -> ZoneState {
        zone_state(
            &self.status,
            self.frame.is_some(),
            now.saturating_duration_since(self.last_frame_at),
        )
    }

    /// How long the current connection attempt has been running.
    fn connecting_for(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.connecting_since)
    }

    /// What the decoder said, in its own words.
    fn fault(&self) -> &str {
        match &self.status {
            Status::Error(message) => message,
            _ => "",
        }
    }

    /// The decoder's own words, kept for whoever asks a unit what happened to
    /// it. The sheet prints its own words and keeps ffmpeg's for the tooltip.
    fn fault_detail(&self) -> Option<SharedString> {
        match self.fault() {
            "" => None,
            message => Some(
                format!("{} · {}", fault_reason(message), message)
                    .to_uppercase()
                    .into(),
            ),
        }
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

/// The host a camera is reached at, as it is printed on the sheet.
///
/// The address in the configuration carries credentials and a stream path.
/// The sheet prints neither: an address on a sheet is a place, and a password
/// on a sheet is a password on a sheet.
fn host_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);

    let host = match host_port.split_once(']') {
        // A literal IPv6 address keeps its brackets and drops its port.
        Some((address, _)) => format!("{address}]"),
        None => host_port.split(':').next().unwrap_or(host_port).to_string(),
    };

    if host.is_empty() {
        "—".to_string()
    } else {
        host
    }
}

struct Camera {
    id: u64,
    name: SharedString,
    /// The host, for the sheet's secondary column.
    host: SharedString,
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
    /// Lets the keyboard reach the unit.
    focus: FocusHandle,
}

impl Camera {
    /// The feed to draw right now.
    ///
    /// The full-resolution feed is drawn only while it is actually running. A
    /// soft picture from the wall feed beats a frozen full-resolution one, and
    /// the caption describes whichever feed is drawn.
    fn feed(&self, now: Instant, enlarged: bool) -> &Feed {
        match &self.solo {
            Some(solo) if draw_solo(enlarged, solo.state(now) == ZoneState::Live) => solo,
            _ => &self.wall,
        }
    }

    /// What the sheet prints for this camera, for the feed it is drawing.
    fn state(&self, now: Instant, enlarged: bool) -> ZoneState {
        self.feed(now, enlarged).state(now)
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

    fn poll(&mut self, now: Instant, retired: &mut Vec<Arc<RenderImage>>) -> bool {
        let mut changed = self.wall.poll(now, retired);
        if let Some(solo) = &mut self.solo {
            changed |= solo.poll(now, retired);
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
    /// The box the frame gave this view, measured from the frame itself on the
    /// previous tick. The window surface is bigger than this wherever the frame
    /// keeps its shadow and its border on the inside. See `sheet_box`.
    frame: Rc<Cell<(f32, f32)>>,
    /// A box a test has forced, which the frame's own measurement does not
    /// replace. `None` in the app.
    forced_frame: Option<(f32, f32)>,
    show_add: bool,
    name_input: Entity<InputState>,
    hd_input: Entity<InputState>,
    sd_input: Entity<InputState>,
    add_error: Option<SharedString>,
    /// Where the camera list is saved. `None` in a test, which keeps the
    /// list in memory and never touches the user's file.
    config_path: Option<PathBuf>,
    update: UpdateState,
    /// The version the user has already been told about in this run, so the
    /// same offer is never made twice.
    offered: Option<String>,
    /// Notes from the build that just replaced the old one, shown once.
    whats_new: Option<WhatsNew>,
    audio: Option<AudioEngine>,
    /// Frames replaced this tick. Dropped on the next tick, once the GPU has
    /// finished with them.
    retired: Vec<Arc<RenderImage>>,
    _subscriptions: Vec<Subscription>,
    _tick: Task<()>,
    _update_watch: Option<Task<()>>,
}

impl CamViewer {
    /// The view in normal use: the saved cameras, and a look for a newer
    /// release.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(window, cx, config::load(), Some(config::path()))
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

        // The sheet holds one appearance, so the window does not follow the
        // operating system's. Only the operating system's own window frame does.
        paint_the_sheet(window, cx);
        let mut subscriptions: Vec<Subscription> = Vec::new();

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

        // The update watch belongs to the real app only. A test keeps its
        // cameras in memory and must not open a socket.
        let update_watch = config_path.as_ref().map(|_| {
            cx.spawn(async move |this, cx| {
                loop {
                    let outcome = match cx
                        .background_spawn(async { update::check(update::current_version()) })
                        .await
                    {
                        Ok(Some(release)) => {
                            if this.update(cx, |this, cx| this.offer(release, cx)).is_err() {
                                break;
                            }
                            CheckOutcome::Answered
                        }
                        Ok(None) => CheckOutcome::Answered,
                        Err(_) => CheckOutcome::Failed,
                    };

                    cx.background_executor()
                        .timer(wait_before_next_check(outcome))
                        .await;

                    if this.update(cx, |_, _| {}).is_err() {
                        break;
                    }
                }
            })
        });

        let mut this = Self {
            cameras: Vec::new(),
            next_id: 1,
            enlarged: None,
            frame: Rc::new(Cell::new((0.0, 0.0))),
            forced_frame: None,
            show_add: false,
            name_input,
            hd_input,
            sd_input,
            add_error: None,
            config_path: config_path.clone(),
            update: UpdateState::Idle,
            offered: None,
            whats_new: config_path.as_ref().and_then(|_| config::load_whats_new()),
            audio: None,
            retired: Vec::new(),
            _subscriptions: subscriptions,
            _tick: tick,
            _update_watch: update_watch,
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

        let now = Instant::now();
        for camera in &mut self.cameras {
            changed |= camera.poll(now, &mut retired);
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
            host: host_of(&url_hd).into(),
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

    /// Puts a newer build on the sheet.
    ///
    /// A check that fails stays quiet: a missing network must not nag, and the
    /// next check comes soon enough. What it must not do is disappear, which is
    /// why the watch retries rather than asking once and giving up.
    fn offer(&mut self, release: Release, cx: &mut Context<Self>) {
        if !should_offer(&release.version, self.offered.as_deref()) {
            return;
        }
        // A download already in flight is never interrupted by a fresh offer.
        if matches!(self.update, UpdateState::Downloading(_)) {
            return;
        }

        self.offered = Some(release.version.clone());
        self.update = UpdateState::Available(release);
        cx.notify();
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

    /// The box this view is drawn in.
    ///
    /// `Window::viewport_size` is the whole window surface, and the drawing
    /// frame keeps its shadow and its border inside that surface. Measuring the
    /// frame's own box and laying the sheet out in it is what keeps the last row
    /// of the wall inside the window: against the surface it hung out of the
    /// bottom, where the frame cropped it, and every unit was squeezed by the
    /// same amount.
    fn sheet_box(&self, window: &Window) -> (f32, f32) {
        if let Some((width, height)) = self.forced_frame {
            return (width, height);
        }

        let measured = self.frame.get();
        if measured.0 >= 1.0 && measured.1 >= 1.0 {
            return measured;
        }

        // The first frame of a window, which has nothing measured yet: take the
        // frame off the surface by what the window says it holds.
        let viewport = window.viewport_size();
        inside_frame(
            f32::from(viewport.width),
            f32::from(viewport.height),
            window_paddings(window),
        )
    }

    /// Hands the view a smaller box than the window, and keeps it there, so a
    /// test can lay out the sheet the way a decorated window does. Tests only: a
    /// headless window has no frame, so nothing else produces that box.
    pub fn set_frame_for_test(&mut self, width: f32, height: f32) {
        self.forced_frame = Some((width, height));
    }

    /// The height of everything between the title block and the wall: the strip
    /// a new release adds, and nothing else.
    fn chrome_h(&self) -> f32 {
        match self.update {
            UpdateState::Idle => 0.0,
            _ => STRIP_H,
        }
    }

    /// The title block: what this sheet is, and every control on it.
    fn render_title_block(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        let width = self.sheet_box(window).0;
        let furniture = sheet_furniture(width);
        let count = self.cameras.len();
        let columns = grid_columns(count.max(1));
        let rows = count.max(1).div_ceil(columns);
        let enlarged = self.enlarged.is_some();

        let mut brand = v_flex()
            .min_w_0()
            .child(
                div()
                    .text_size(px(25.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("CAMVIEWER"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MUTED))
                    .child("RACK ELEVATION"),
            );

        // The revision of a drawing is the version of the app. It belongs on the
        // sheet, where a reader looks for it, and nowhere else.
        if furniture.meta {
            brand = brand.child(
                mono_word(
                    cx,
                    format!(
                        "SHEET 01 · {count} UNITS · {columns} × {rows} · REV {}",
                        update::current_version()
                    ),
                    11.0,
                    MUTED,
                )
                .mt(px(3.0)),
            );
        }

        let mut controls = h_flex().flex_none().gap_4().items_center();
        if furniture.legend {
            controls = controls.child(self.render_legend(cx));
        }
        if enlarged {
            controls = controls.child(sheet_button(
                ("show-all", 0),
                "All cameras",
                Tone::Outline,
                cx.listener(|this, _, _, cx| this.set_enlarged(None, cx)),
            ));
        }
        controls = controls.child(sheet_button(
            ("full-screen", 0),
            if window.is_fullscreen() {
                "Exit full screen"
            } else {
                "Full screen"
            },
            Tone::Outline,
            cx.listener(|_this, _, window, cx| {
                window.toggle_fullscreen();
                cx.notify();
            }),
        ));
        controls = controls.child(sheet_button(
            ("add-camera", 0),
            "+ Add camera",
            // One filled control per view. While the sheet is empty that is the
            // one on the sheet itself, so this one steps back.
            if count == 0 {
                Tone::Outline
            } else {
                Tone::Filled
            },
            cx.listener(|this, _, window, cx| this.open_add(window, cx)),
        ));

        h_flex()
            .w_full()
            .flex_none()
            .h(px(TITLE_H))
            .px(px(PAD))
            .justify_between()
            .items_center()
            .border_b(px(RULE_W))
            .border_color(rgb(GRAPHITE))
            .child(brand)
            .child(controls)
    }

    /// The three state symbols with the words for them.
    ///
    /// A symbol vocabulary is honest only when the sheet names it once.
    fn render_legend(&self, cx: &Context<Self>) -> Div {
        let mut legend = h_flex().flex_none().gap_5().items_center();
        for state in [ZoneState::Live, ZoneState::Connecting, ZoneState::NoSignal] {
            legend = legend.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Self::state_symbol(14.0, symbol_for(state), GRAPHITE))
                    .child(mono_word(cx, state.word().to_uppercase(), 11.0, MUTED)),
            );
        }
        legend
    }

    /// The state symbol, drawn the same way in the legend and in a caption: a
    /// filled square for a unit that is live, a half-filled one for a
    /// connection in progress, a hollow one for a slot with nothing in it.
    fn state_symbol(size: f32, fill: SymbolFill, colour: u32) -> Div {
        let mut symbol = div()
            .flex_none()
            .w(px(size))
            .h(px(size))
            .border(px(HAIRLINE_W))
            .border_color(rgb(colour));

        match fill {
            SymbolFill::Full => symbol = symbol.bg(rgb(colour)),
            SymbolFill::Half => {
                symbol = symbol.child(div().w(px((size - 2.0) / 2.0)).h_full().bg(rgb(colour)));
            }
            SymbolFill::None => {}
        }

        symbol
    }

    /// The measured width of one unit, ruled over the wall the way a drawing
    /// dimensions what it is showing.
    fn render_dimension_row(&self, cx: &Context<Self>, value: String) -> Div {
        let mut row = div().relative().flex_none().w_full().h(px(DIM_H));

        // Extension lines drop toward the wall's own corners.
        for side in 0..2 {
            let mut line = div()
                .absolute()
                .top(px(7.0))
                .bottom_0()
                .w(px(HAIRLINE_W))
                .bg(rgb(GRAPHITE));
            line = if side == 0 {
                line.left_0()
            } else {
                line.right_0()
            };
            row = row.child(line);
        }

        // The dimension line, with a tick at each end rather than an arrowhead.
        row = row
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(14.0))
                    .h(px(HAIRLINE_W))
                    .bg(rgb(GRAPHITE)),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(12.0))
                    .w(px(7.0))
                    .h(px(5.0))
                    .bg(rgb(GRAPHITE)),
            )
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top(px(12.0))
                    .w(px(7.0))
                    .h(px(5.0))
                    .bg(rgb(GRAPHITE)),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top_0()
                    .flex()
                    .justify_center()
                    .child(mono_word(cx, value, 11.0, GRAPHITE).px_2().bg(rgb(VELLUM))),
            );

        row
    }

    /// The margin gutter: one row reference for each row of the wall.
    fn render_gutter(&self, cx: &Context<Self>, rows: usize) -> Div {
        let mut gutter = v_flex()
            .flex_1()
            .min_h_0()
            .gap(px(GAP))
            .border_r(px(HAIRLINE_W))
            .border_color(rgb(NON_REPRO))
            .pr(px(8.0));

        for index in 0..rows {
            gutter = gutter.child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(mono_word(cx, row_label(index), 11.0, MUTED))
                    .child(div().w(px(9.0)).h(px(HAIRLINE_W)).bg(rgb(GRAPHITE))),
            );
        }

        gutter
    }

    /// The sheet's body: the dimension row, the numbered gutter and the wall.
    fn render_sheet(&self, cx: &Context<Self>, wall: Div, gutter: Div, dim: String) -> Div {
        h_flex()
            .size_full()
            .px(px(PAD))
            .pt(px(BODY_TOP))
            .pb(px(PAD))
            .gap(px(GAP))
            .child(
                // The gutter starts below the dimension row, so its numbers line
                // up with the wall rather than with the edge of the sheet.
                v_flex()
                    .flex_none()
                    .w(px(MARGIN_W))
                    .h_full()
                    .child(div().flex_none().h(px(DIM_H)))
                    .child(gutter),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_dimension_row(cx, dim))
                    .child(wall),
            )
    }

    /// The wall, as a grid of units. Returns the wall and how many rows it has,
    /// which is what the margin gutter numbers.
    fn render_wall(
        &self,
        now: Instant,
        box_w: f32,
        box_h: f32,
        cx: &Context<Self>,
    ) -> (Div, usize) {
        let (avail_w, avail_h) = wall_box(box_w, box_h, self.chrome_h());
        let count = self.cameras.len();

        // Nothing on the sheet yet: one empty slot, at the full size of the wall,
        // so the first thing a new user sees is where a camera goes.
        if count == 0 {
            let wall = v_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(self.render_unassigned(0, avail_w, avail_h, true, cx));
            return (wall, 1);
        }

        let columns = grid_columns(count);
        let (unit_w, unit_h) = tile_size(columns, count, avail_w, avail_h);
        let mut wall = v_flex().flex_1().min_h_0().w_full().gap(px(GAP));
        let mut rows = 0usize;
        let mut slot = 0usize;

        for row in self.cameras.chunks(columns) {
            let mut cells: Vec<AnyElement> = row
                .iter()
                .map(|camera| self.render_unit(camera, now, unit_w, unit_h, false, cx))
                .collect();

            // A short last row keeps the grid complete, and an empty slot is the
            // wall's own way to add the next camera.
            for _ in row.len()..columns {
                cells.push(self.render_unassigned(slot, unit_w, unit_h, false, cx));
                slot += 1;
            }

            rows += 1;
            wall = wall.child(h_flex().flex_none().gap(px(GAP)).children(cells));
        }

        (wall, rows)
    }

    /// One camera's slot on the sheet.
    fn render_unit(
        &self,
        camera: &Camera,
        now: Instant,
        width: f32,
        height: f32,
        enlarged: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = camera.id;
        let state = camera.state(now, false);
        let feed = camera.feed(now, false);
        let ink = if state.is_fault() { FAULT } else { GRAPHITE };
        let unit_box = UnitBox { width, height };
        let (inner_w, inner_h) = unit_box.picture();

        let mut unit = div()
            .id(("unit", id))
            .group(SharedString::from(format!("unit-{id}")))
            .flex()
            .flex_col()
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .border(px(RULE_W))
            .border_color(rgb(ink))
            .bg(rgb(PLATE))
            .cursor_pointer()
            .track_focus(&camera.focus)
            .tab_stop(true)
            // On the wall a unit enlarges its camera; drawn alone, the same
            // action is the way back, which is what the picture already means.
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_enlarged(if enlarged { None } else { Some(id) }, cx)
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    this.set_enlarged(if enlarged { None } else { Some(id) }, cx);
                }
            }));

        // What the sheet keeps off the caption, for whoever asks a unit about
        // itself: the address it was reached at, and what the decoder said.
        let mut detail = format!("{} · {}", camera.name, camera.host).to_uppercase();
        if let Some(fault) = feed.fault_detail() {
            detail = format!("{detail} · {fault}");
        }
        let detail: SharedString = detail.into();
        unit = unit.tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx));

        unit.child(
            div()
                .flex_none()
                .w(px(inner_w))
                .h(px(inner_h))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(PICTURE))
                .child(self.render_picture(feed, state, inner_w, inner_h)),
        )
        .child(
            self.render_caption(camera, state, feed, now, unit_box, cx)
                // A focused unit is marked on its caption rather than by a ring,
                // because a ring outside the unit would move the wall.
                .into_any_element(),
        )
        .test_support()
        .into_any_element()
    }

    /// A slot with no camera in it. It invites the next one instead of leaving a
    /// hole in the sheet.
    fn render_unassigned(
        &self,
        index: usize,
        width: f32,
        height: f32,
        first_run: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let inner_w = (width - 2.0 * RULE_W).max(1.0);
        let inner_h = (height - 2.0 * RULE_W - CAPTION_H).max(1.0);

        let mut field = v_flex()
            .w(px(inner_w))
            .h(px(inner_h))
            .items_center()
            .justify_center()
            .gap_3()
            // The empty slot is the sheet's paper, not a plate: it is a space
            // waiting for a drawing, and it must recede behind the units.
            .bg(rgb(VELLUM));

        if first_run {
            field = field
                .child(
                    div()
                        .text_size(px(28.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child("NO CAMERAS ON THIS SHEET"),
                )
                .child(mono_word(
                    cx,
                    "ADD THE RTSP ADDRESS OF A CAMERA TO BEGIN",
                    11.0,
                    MUTED,
                ));
        }

        field = field.child(sheet_button(
            ("add-slot", index as u64),
            "+ Add camera",
            if first_run {
                Tone::Filled
            } else {
                Tone::Outline
            },
            cx.listener(|this, _, window, cx| this.open_add(window, cx)),
        ));

        div()
            .id(("slot", index as u64))
            .flex()
            .flex_col()
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .border(px(RULE_W))
            .border_color(rgb(GRAPHITE))
            .bg(rgb(PLATE))
            .child(field)
            .child(
                h_flex()
                    .flex_none()
                    .w(px(inner_w))
                    .h(px(CAPTION_H))
                    .px_3()
                    .gap_3()
                    .items_center()
                    .border_t(px(RULE_W))
                    .border_color(rgb(GRAPHITE))
                    .bg(rgb(PLATE))
                    .child(
                        div()
                            .text_size(px(if width < COMPACT_BELOW { 15.0 } else { 21.0 }))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MUTED))
                            .child("UNASSIGNED"),
                    )
                    .child(
                        h_flex().flex_none().ml_auto().items_center().child(
                            v_flex()
                                .flex_none()
                                .items_center()
                                .gap(px(2.0))
                                .child(Self::state_symbol(13.0, SymbolFill::None, GRAPHITE))
                                .child(div().w(px(15.0)).h(px(HAIRLINE_W)).bg(rgb(GRAPHITE))),
                        ),
                    ),
            )
            .test_support()
            .into_any_element()
    }

    /// A unit's caption: the name, the host, and the state the sheet prints for
    /// this camera. The state is never hidden behind a hover.
    fn render_caption(
        &self,
        camera: &Camera,
        state: ZoneState,
        feed: &Feed,
        now: Instant,
        unit: UnitBox,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = camera.id;
        let ink = if state.is_fault() { FAULT } else { GRAPHITE };
        let compact = unit.compact();
        let wide = unit.is_wide();
        let word_colour = match state {
            ZoneState::Live => GRAPHITE,
            ZoneState::Connecting => MUTED,
            ZoneState::Frozen | ZoneState::NoSignal => FAULT,
        };

        let mut caption = h_flex()
            .flex_none()
            .w(px(unit.picture().0))
            .h(px(CAPTION_H))
            .px_3()
            .gap_3()
            .items_center()
            .border_t(px(RULE_W))
            .border_color(rgb(ink))
            .bg(rgb(PLATE))
            .focus_visible(|style| style.bg(rgb(0xe4e4e1)))
            .child(
                div()
                    .flex_none()
                    // The name keeps its room, up to half the caption. A name
                    // that long was never going to fit, and a truncated name is
                    // still a name.
                    .max_w(px((unit.picture().0 * 0.5).max(80.0)))
                    .truncate()
                    .text_size(px(if compact { 15.0 } else { 21.0 }))
                    .font_weight(FontWeight::MEDIUM)
                    .child(camera.name.to_uppercase()),
            );

        // The address is the sheet's secondary column, so it is what gives way:
        // a name that loses its last letters is a different camera, and the
        // state is what the wall exists for.
        if wide {
            caption = caption.child(
                mono_word(cx, camera.host.clone(), 11.0, MUTED)
                    .flex_1()
                    .min_w_0()
                    .truncate(),
            );
        }

        let mut trailing = h_flex().flex_none().gap_3().items_center();

        // A unit short of room hands its controls to the hover, so that the name
        // and the state keep their room.
        if wide {
            trailing = trailing.child(sheet_button(
                ("sound", id),
                if camera.muted { "Muted" } else { "Sound" },
                Tone::Outline,
                cx.listener(move |this, _, _, cx| this.toggle_mute(id, cx)),
            ));
            trailing = trailing.child(self.render_remove(camera, cx));
        } else {
            trailing = trailing.child(self.render_hidden_controls(camera, cx));
        }

        trailing = trailing
            .child(mono_word(
                cx,
                state_words(state, feed.connecting_for(now)),
                11.0,
                word_colour,
            ))
            .child(
                v_flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.0))
                    .child(Self::state_symbol(13.0, symbol_for(state), ink))
                    .child(div().w(px(15.0)).h(px(HAIRLINE_W)).bg(rgb(ink))),
            );

        caption.child(trailing).into_any_element()
    }

    /// The sound and remove controls, waiting behind a hover so that a small
    /// unit can spend its width on the name and the state instead.
    fn render_hidden_controls(&self, camera: &Camera, cx: &Context<Self>) -> AnyElement {
        let id = camera.id;
        div()
            .id(("controls-slot", id))
            .flex_none()
            .opacity(0.0)
            .group_hover(SharedString::from(format!("unit-{id}")), |style| {
                style.opacity(1.0)
            })
            .child(
                h_flex()
                    .flex_none()
                    .gap_3()
                    .items_center()
                    .child(sheet_button(
                        ("sound", id),
                        if camera.muted { "Muted" } else { "Sound" },
                        Tone::Quiet,
                        cx.listener(move |this, _, _, cx| this.toggle_mute(id, cx)),
                    ))
                    .child(sheet_button(
                        ("remove", id),
                        "Remove",
                        Tone::Quiet,
                        cx.listener(move |this, _, _, cx| this.remove_camera(id, cx)),
                    )),
            )
            .into_any_element()
    }

    /// Removing a camera stays hidden until its unit is hovered, so it can never
    /// be a slip of the hand.
    fn render_remove(&self, camera: &Camera, cx: &Context<Self>) -> AnyElement {
        let id = camera.id;
        div()
            .id(("remove-slot", id))
            .flex_none()
            .opacity(0.0)
            .group_hover(SharedString::from(format!("unit-{id}")), |style| {
                style.opacity(1.0)
            })
            .child(sheet_button(
                ("remove", id),
                "Remove",
                Tone::Quiet,
                cx.listener(move |this, _, _, cx| this.remove_camera(id, cx)),
            ))
            .into_any_element()
    }

    /// What a unit draws in its picture field.
    fn render_picture(&self, feed: &Feed, state: ZoneState, width: f32, height: f32) -> AnyElement {
        // A frozen stream keeps its last frame: that picture is what the camera
        // last saw, and the caption is what says it is not moving. Drawing
        // nothing there would hide the fault instead of reporting it.
        if matches!(state, ZoneState::Live | ZoneState::Frozen)
            && let Some(frame) = &feed.frame
        {
            return video(frame.clone(), width, height);
        }

        let (word, colour) = match state {
            ZoneState::NoSignal => (fault_reason(feed.fault()).to_uppercase(), NON_REPRO),
            ZoneState::Live | ZoneState::Frozen => ("NO PICTURE YET".to_string(), QUIET_ON_PICTURE),
            _ => ("CONNECTING…".to_string(), QUIET_ON_PICTURE),
        };

        v_flex()
            .w(px(width))
            .h(px(height))
            .items_center()
            .justify_center()
            .px_4()
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(colour))
                    .child(word),
            )
            .into_any_element()
    }

    /// One camera drawn alone, on the same sheet: the dimension row, the gutter
    /// and the unit stay exactly where they were.
    fn render_enlarged(
        &self,
        id: u64,
        now: Instant,
        box_w: f32,
        box_h: f32,
        cx: &Context<Self>,
    ) -> Div {
        let Some(camera) = self.cameras.iter().find(|camera| camera.id == id) else {
            return div();
        };

        let (avail_w, avail_h) = wall_box(box_w, box_h, self.chrome_h());
        let (unit_w, unit_h) = (avail_w.max(1.0), avail_h.max(1.0));

        let wall = v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(self.render_unit(camera, now, unit_w, unit_h, true, cx));

        let gutter = v_flex()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .border_r(px(HAIRLINE_W))
            .border_color(rgb(NON_REPRO))
            .child(mono_word(cx, row_label(0), 11.0, MUTED));

        self.render_sheet(cx, wall, gutter, format!("{unit_w:.0} × {unit_h:.0}"))
    }

    /// A strip under the title block while a newer build is on offer.
    fn render_update_strip(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let (message, actions): (SharedString, Vec<AnyElement>) = match &self.update {
            UpdateState::Idle => return None,
            UpdateState::Available(release) => (
                format!(
                    "AMENDMENT · CAMVIEWER {} IS READY TO INSTALL",
                    release.version
                )
                .into(),
                vec![
                    sheet_button(
                        ("update-later", 0),
                        "Later",
                        Tone::Outline,
                        cx.listener(|this, _, _, cx| {
                            this.update = UpdateState::Idle;
                            cx.notify();
                        }),
                    ),
                    sheet_button(
                        ("update-install", 0),
                        "Install and restart",
                        Tone::Outline,
                        cx.listener(|this, _, _, cx| this.install_update(cx)),
                    ),
                ],
            ),
            UpdateState::Downloading(release) => (
                format!("AMENDMENT · DOWNLOADING CAMVIEWER {}…", release.version).into(),
                Vec::new(),
            ),
            UpdateState::Failed(message) => (
                format!("AMENDMENT · {message}").into(),
                vec![sheet_button(
                    ("update-dismiss", 0),
                    "Dismiss",
                    Tone::Outline,
                    cx.listener(|this, _, _, cx| {
                        this.update = UpdateState::Idle;
                        cx.notify();
                    }),
                )],
            ),
        };

        let mut strip = h_flex()
            .w_full()
            .flex_none()
            .h(px(STRIP_H))
            .px(px(PAD))
            .gap_4()
            .items_center()
            .bg(rgb(PLATE))
            .border_b(px(RULE_W))
            .border_color(rgb(GRAPHITE))
            .child(mono_word(cx, message, 12.0, GRAPHITE).min_w_0().truncate());

        strip = strip.child(h_flex().flex_none().ml_auto().gap_3().children(actions));

        Some(strip.into_any_element())
    }

    /// What changed in the build that was just installed. Shown once.
    fn render_whats_new(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
            .bg(rgba(0x1a1c1ecc))
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
                    .gap_4()
                    .border(px(RULE_W))
                    .border_color(rgb(GRAPHITE))
                    .bg(rgb(PLATE))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        v_flex()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(22.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("REVISION NOTES"),
                            )
                            .child(mono_word(
                                cx,
                                format!("CAMVIEWER {version} · FILED ONCE, ON FIRST START"),
                                11.0,
                                MUTED,
                            )),
                    )
                    .child(
                        div()
                            .id("whats-new-notes")
                            .max_h(px(340.0))
                            .overflow_y_scroll()
                            .child(TextView::markdown("whats-new-body", notes)),
                    )
                    .child(h_flex().w_full().justify_end().child(sheet_button(
                        ("whats-new-ok", 0),
                        "Filed",
                        Tone::Filled,
                        dismiss,
                    ))),
            )
            .into_any_element()
    }

    /// The form that adds a camera, drawn as the sheet's own record card.
    fn render_add_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let field = |cx: &Context<Self>,
                     label: &'static str,
                     id: &'static str,
                     note: Option<&'static str>,
                     input: &Entity<InputState>| {
            let mut block = v_flex()
                .gap(px(5.0))
                .child(mono_word(cx, label, 11.0, MUTED))
                .child(Input::new(input).id(id).cleanable(true));

            if let Some(note) = note {
                block = block.child(div().text_size(px(11.0)).text_color(rgb(MUTED)).child(note));
            }

            block
        };

        let mut dialog = v_flex()
            .id("add-camera-dialog")
            .occlude()
            .w(px(520.0))
            .p_5()
            .gap_5()
            .border(px(RULE_W))
            .border_color(rgb(GRAPHITE))
            .bg(rgb(PLATE))
            // A click inside the card must not reach the backdrop below.
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                v_flex()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("ADD UNIT"),
                    )
                    .child(mono_word(
                        cx,
                        format!(
                            "SHEET 01 · {} UNITS · REV {}",
                            self.cameras.len(),
                            update::current_version()
                        ),
                        11.0,
                        MUTED,
                    )),
            )
            .child(field(cx, "UNIT NAME", "name-input", None, &self.name_input))
            .child(field(
                cx,
                "HIGH-RESOLUTION RTSP ADDRESS",
                "hd-input",
                None,
                &self.hd_input,
            ))
            .child(field(
                cx,
                "LOW-RESOLUTION RTSP ADDRESS · OPTIONAL",
                "sd-input",
                Some("The wall shows this stream to save bandwidth. Enlarging a camera switches to the address above."),
                &self.sd_input,
            ));

        if let Some(error) = &self.add_error {
            dialog = dialog.child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(FAULT))
                    .child(error.clone()),
            );
        }

        dialog = dialog.child(
            h_flex()
                .w_full()
                .justify_end()
                .gap_3()
                .child(sheet_button(
                    ("cancel-add", 0),
                    "Cancel",
                    Tone::Outline,
                    cx.listener(|this, _, window, cx| this.close_add(window, cx)),
                ))
                .child(sheet_button(
                    ("confirm-add", 0),
                    "Add",
                    Tone::Filled,
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
            .bg(rgba(0x1a1c1ecc))
            .on_click(cx.listener(|this, _, window, cx| this.close_add(window, cx)))
            .child(dialog)
            .into_any_element()
    }
}

impl Render for CamViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let strip = self.render_update_strip(cx);
        let (box_w, box_h) = self.sheet_box(window);

        let body = match self.enlarged {
            Some(id) => self.render_enlarged(id, now, box_w, box_h, cx),
            None => {
                let (wall, rows) = self.render_wall(now, box_w, box_h, cx);
                let (avail_w, avail_h) = wall_box(box_w, box_h, self.chrome_h());
                let count = self.cameras.len();
                let (unit_w, unit_h) = if count == 0 {
                    (avail_w, avail_h)
                } else {
                    tile_size(grid_columns(count), count, avail_w, avail_h)
                };
                let total: usize = rows;
                let gutter = self.render_gutter(cx, total);
                self.render_sheet(cx, wall, gutter, format!("{unit_w:.0} × {unit_h:.0}"))
            }
        };

        // The frame's own box, for the next tick to lay the sheet out in. It
        // cannot be read from the window, which reports the whole surface and
        // not the space the frame leaves inside it, and those few pixels are
        // exactly the bottom row of the wall.
        //
        // The measurement arrives after the layout that needed it, so a box
        // that is not the one the wall was just drawn in owes the window one
        // more frame. Without that a wall nothing is moving on keeps the tile
        // size of the window it used to have.
        let frame = self.frame.clone();
        let view = cx.entity_id();
        let mut root = v_flex()
            .relative()
            .size_full()
            .bg(rgb(VELLUM))
            .text_color(rgb(GRAPHITE))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let measured =
                            (f32::from(bounds.size.width), f32::from(bounds.size.height));
                        if frame.get() != measured {
                            frame.set(measured);
                            cx.notify(view);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(self.render_title_block(window, cx))
            .children(strip)
            .child(div().flex_1().min_h_0().w_full().child(body));

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
    use super::{
        CAPTION_H, CHECK_EVERY, CheckOutcome, FROZEN_AFTER, GAP, RETRY_AFTER, RULE_W, SymbolFill,
        UnitBox, ZoneState, draw_solo, elapsed_label, fault_reason, grid_columns, host_of,
        inside_frame, new_camera, row_label, sheet_furniture, should_offer, state_words,
        symbol_for, tile_size, wait_before_next_check, wall_box, zone_state,
    };
    use crate::stream::Status;
    use gpui_kit::{Edges, px};
    use std::time::Duration;

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
        assert!((w - (1600.0 - GAP) / 2.0).abs() < 0.01, "{w}");
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
    fn the_wall_gives_up_room_to_the_sheet_furniture() {
        // The wall is what is left after the title block, the dimension row and
        // the page margin, and it is never allowed to collapse.
        let (w, h) = wall_box(1440.0, 900.0, 0.0);
        assert!(w < 1440.0 && h < 900.0, "{w}x{h}");

        let (_, stripped) = wall_box(1440.0, 900.0, 42.0);
        assert!((h - stripped - 42.0).abs() < 0.01, "{h} and {stripped}");
    }

    #[test]
    fn a_unit_gives_its_rule_and_its_caption_their_room() {
        let (w, h) = UnitBox {
            width: 440.0,
            height: 355.0,
        }
        .picture();
        assert!((w - (440.0 - 2.0 * RULE_W)).abs() < 0.01, "{w}");
        assert!((h - (355.0 - 2.0 * RULE_W - CAPTION_H)).abs() < 0.01, "{h}");
    }

    #[test]
    fn a_narrow_unit_drops_its_secondary_column() {
        assert!(
            !UnitBox {
                width: 900.0,
                height: 400.0
            }
            .compact()
        );
        assert!(
            UnitBox {
                width: 200.0,
                height: 400.0
            }
            .compact()
        );
    }

    #[test]
    fn a_unit_short_of_room_puts_its_address_and_controls_away() {
        // A two-camera wall on a 1440-wide window: room for both.
        assert!(
            UnitBox {
                width: 657.0,
                height: 746.0
            }
            .is_wide()
        );
        // A five-camera wall on the same window: neither fits, so neither is
        // printed half way.
        assert!(
            !UnitBox {
                width: 435.0,
                height: 368.0
            }
            .is_wide()
        );
    }

    #[test]
    fn a_window_too_small_for_the_sheet_still_yields_a_drawable_wall() {
        let (w, h) = wall_box(1.0, 1.0, 42.0);
        assert!(w >= 1.0 && h >= 1.0, "{w}x{h}");
    }

    // ------------------------------------------------------------- the updates

    #[test]
    fn a_check_that_could_not_reach_github_is_retried_sooner() {
        // The first check runs a second after the window opens, which is when a
        // machine's network may still be settling. Waiting six hours to learn
        // that it failed is how a wall misses a release for a day.
        let retry = wait_before_next_check(CheckOutcome::Failed);
        assert_eq!(retry, RETRY_AFTER);
        assert!(
            retry < CHECK_EVERY,
            "{retry:?} is not sooner than {CHECK_EVERY:?}"
        );
        assert!(
            retry <= Duration::from_secs(15 * 60),
            "{retry:?} is no retry"
        );

        // A wall stays open for weeks, so a check that answered still asks again.
        assert_eq!(wait_before_next_check(CheckOutcome::Answered), CHECK_EVERY);
        assert!(CHECK_EVERY <= Duration::from_secs(12 * 60 * 60));
    }

    #[test]
    fn the_same_release_is_never_offered_twice() {
        assert!(should_offer("0.4.1", None));
        assert!(
            !should_offer("0.4.1", Some("0.4.1")),
            "saying Later was an answer"
        );
        assert!(
            should_offer("0.5.0", Some("0.4.1")),
            "a newer build brings its own offer"
        );
    }

    // ------------------------------------------------------------- the states

    #[test]
    fn a_connecting_stream_is_not_reported_as_live() {
        assert_eq!(
            zone_state(&Status::Connecting, false, Duration::ZERO),
            ZoneState::Connecting
        );
    }

    #[test]
    fn a_refused_stream_is_reported_as_having_no_signal() {
        assert_eq!(
            zone_state(&Status::Error("404".into()), false, Duration::ZERO),
            ZoneState::NoSignal
        );
    }

    #[test]
    fn a_live_stream_that_has_drawn_nothing_is_still_connecting() {
        assert_eq!(
            zone_state(&Status::Live, false, Duration::ZERO),
            ZoneState::Connecting
        );
    }

    #[test]
    fn a_live_stream_that_stops_drawing_is_reported_as_frozen() {
        // The whole reason this wall exists: a still picture must not pass for
        // a moving one.
        assert_eq!(
            zone_state(&Status::Live, true, FROZEN_AFTER + Duration::from_millis(1)),
            ZoneState::Frozen
        );
    }

    #[test]
    fn a_live_stream_that_is_drawing_is_live() {
        assert_eq!(
            zone_state(&Status::Live, true, Duration::ZERO),
            ZoneState::Live
        );
        assert_eq!(
            zone_state(&Status::Live, true, FROZEN_AFTER - Duration::from_millis(1)),
            ZoneState::Live
        );
    }

    #[test]
    fn only_a_fault_is_worth_the_one_colour_the_sheet_has() {
        assert!(ZoneState::NoSignal.is_fault());
        assert!(ZoneState::Frozen.is_fault());
        assert!(!ZoneState::Live.is_fault());
        assert!(!ZoneState::Connecting.is_fault());
    }

    #[test]
    fn the_state_symbols_differ_one_from_another() {
        // The legend names three symbols once, so no two of them may look alike.
        assert_eq!(symbol_for(ZoneState::Live), SymbolFill::Full);
        assert_eq!(symbol_for(ZoneState::Connecting), SymbolFill::Half);
        assert_eq!(symbol_for(ZoneState::NoSignal), SymbolFill::None);
        // A frozen stream still has a picture on the sheet: the last one.
        assert_eq!(symbol_for(ZoneState::Frozen), SymbolFill::Full);

        let named = [
            symbol_for(ZoneState::Live),
            symbol_for(ZoneState::Connecting),
            symbol_for(ZoneState::NoSignal),
        ];
        for (index, one) in named.iter().enumerate() {
            for other in named.iter().skip(index + 1) {
                assert_ne!(one, other, "two legend entries look alike");
            }
        }
    }

    #[test]
    fn a_connection_that_is_still_trying_prints_how_long_it_has_been_trying() {
        assert_eq!(
            state_words(ZoneState::Connecting, Duration::from_secs(72)),
            "CONNECTING 01:12"
        );
        assert_eq!(state_words(ZoneState::Live, Duration::ZERO), "LIVE");
        assert_eq!(
            state_words(ZoneState::NoSignal, Duration::ZERO),
            "NO SIGNAL"
        );
    }

    #[test]
    fn elapsed_labels_never_run_past_two_digits_of_minutes() {
        assert_eq!(elapsed_label(Duration::ZERO), "00:00");
        assert_eq!(elapsed_label(Duration::from_secs(59)), "00:59");
        assert_eq!(elapsed_label(Duration::from_secs(3600)), "60:00");
    }

    // -------------------------------------------------------------- the causes

    #[test]
    fn ffmpeg_is_never_quoted_on_the_sheet() {
        // These are the messages the decoder actually produces. None of them is
        // worth printing at 13px on a unit.
        let reported = [
            "[fatal] Error opening Input files: Server returned 404 Not Found",
            "rtsp://192.168.1.10: Operation not permitted",
            "Connection refused",
            "No route to host",
            "[tcp] Connection timed out",
            "Server returned 401 Unauthorized",
            "cannot run ffmpeg: No such file or directory",
        ];

        let printed = [
            "Address not found",
            "Session refused",
            "Connection refused",
            "Host unreachable",
            "Timed out",
            "Password refused",
            "ffmpeg is not installed",
        ];

        for (message, expected) in reported.iter().zip(printed) {
            assert_eq!(fault_reason(message), expected, "{message}");
        }
    }

    #[test]
    fn an_unrecognised_failure_still_says_something_true() {
        assert_eq!(fault_reason("something nobody has seen"), "Stream ended");
        assert_eq!(fault_reason(""), "Stream ended");
    }

    // ------------------------------------------------------------ the addresses

    #[test]
    fn a_sheet_prints_the_host_and_never_the_password() {
        assert_eq!(
            host_of("rtsp://admin:secret@192.168.1.10:554/stream1"),
            "192.168.1.10"
        );
        assert_eq!(host_of("rtsp://cam/1"), "cam");
        assert_eq!(host_of("rtsp://[fd00::1]:554/s1"), "[fd00::1]");
        assert_eq!(host_of("not an address"), "not an address");
        assert_eq!(host_of(""), "—");
    }

    #[test]
    fn rows_are_numbered_from_the_top_the_way_a_drawing_is_read() {
        assert_eq!(row_label(0), "R1");
        assert_eq!(row_label(1), "R2");
        assert_eq!(row_label(11), "R12");
    }

    // -------------------------------------------------------------- the furniture

    #[test]
    fn a_narrow_window_drops_the_sheets_secondary_furniture() {
        let narrow = sheet_furniture(640.0);
        assert!(!narrow.meta && !narrow.legend);

        let wide = sheet_furniture(1440.0);
        assert!(wide.meta && wide.legend);
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
    // ------------------------------------------------------------- the frame

    #[test]
    fn a_window_frame_gives_the_wall_less_room_than_the_window_surface() {
        // The surface carries the frame's shadow and its border, which is what
        // the wall used to be measured against. These are the numbers a 1296 by
        // 1410 surface with 20 px of frame on every side produced.
        let (w, h) = inside_frame(1296.0, 1410.0, Edges::all(px(20.0)));
        assert!((w - 1256.0).abs() < 0.01, "{w}");
        assert!((h - 1370.0).abs() < 0.01, "{h}");

        // A window with no frame keeps every part of its surface.
        let (w, h) = inside_frame(1296.0, 1410.0, Edges::all(px(0.0)));
        assert!((w - 1296.0).abs() < 0.01, "{w}");
        assert!((h - 1410.0).abs() < 0.01, "{h}");

        // A window with no room left still yields a drawable wall.
        let (w, h) = inside_frame(10.0, 10.0, Edges::all(px(20.0)));
        assert!(w >= 1.0 && h >= 1.0, "{w}x{h}");
    }
}
