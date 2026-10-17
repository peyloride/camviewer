//! Video decoding.
//!
//! One background thread runs one `ffmpeg` process per camera. The thread
//! decodes the RTSP stream into BGRA frames and publishes the newest frame into
//! a shared slot. The UI reads the slot on a timer, so a slow repaint drops
//! frames instead of queueing them.

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use gpui_kit::RenderImage;
use image::{Frame, RgbaImage};

use crate::audio::{self, PcmPipe};
use smallvec::SmallVec;

/// Longest edge of a decoded frame, in pixels.
///
/// Grid tiles are small. A full 4K sensor costs four times the decode work and
/// four times the texture upload for detail nobody sees. Raise this for a
/// single-camera wall, lower it for many cameras on a weak CPU.
const MAX_WIDTH: u32 = 1280;

/// Drop a stalled connection after this many microseconds without data.
const IO_TIMEOUT_US: &str = "5000000";

/// Wait before a reconnect attempt after the stream ended.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// How often the reconnect wait checks the stop flag.
const RECONNECT_POLL: Duration = Duration::from_millis(100);

/// How long the frame rate is averaged over before it is published.
const RATE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Live,
    Error(String),
}

/// The newest frame and the current connection state of one camera.
pub struct Slot {
    /// Increases on every published frame or status change.
    pub seq: u64,
    pub frame: Option<Arc<RenderImage>>,
    pub status: Status,
    pub resolution: Option<(u32, u32)>,
    /// Frames per second the decoder is really reading, measured over
    /// [`RATE_WINDOW`]. `None` until a second of frames has arrived.
    pub fps: Option<f32>,
    /// The codec ffmpeg reported for the camera's picture, e.g. `h264`.
    pub codec: Option<String>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            seq: 0,
            frame: None,
            status: Status::Connecting,
            resolution: None,
            fps: None,
            codec: None,
        }
    }
}

/// A running video decoder. Dropping the handle stops the decoder thread.
pub struct VideoStream {
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
    /// The camera's sound, read off the same `ffmpeg` process.
    audio: PcmPipe,
}

impl VideoStream {
    pub fn start(url: String) -> Self {
        let slot = Arc::new(Mutex::new(Slot::default()));
        let stop = Arc::new(AtomicBool::new(false));

        // A loopback socket carries the camera's sound out of the same ffmpeg
        // process that decodes the picture.
        let listener = TcpListener::bind(("127.0.0.1", 0)).ok();
        let port = listener
            .as_ref()
            .and_then(|listener| listener.local_addr().ok())
            .map(|address| address.port());
        let audio = match listener {
            Some(listener) => audio::drain(listener, stop.clone()),
            None => PcmPipe::default(),
        };

        let thread_slot = slot.clone();
        let thread_stop = stop.clone();
        let _ = thread::Builder::new()
            .name("camviewer-video".to_string())
            .spawn(move || decode_loop(&url, port, &thread_slot, &thread_stop));

        Self { slot, stop, audio }
    }

    /// The shared slot that the decoder thread writes into.
    pub fn slot(&self) -> Arc<Mutex<Slot>> {
        self.slot.clone()
    }

    /// The camera's sound, for a player to attach to.
    pub fn audio(&self) -> PcmPipe {
        self.audio.clone()
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        // The thread kills its own ffmpeg child on its next frame.
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn decode_loop(
    url: &str,
    audio_port: Option<u16>,
    slot: &Arc<Mutex<Slot>>,
    stop: &Arc<AtomicBool>,
) {
    // A camera with no sound makes ffmpeg refuse an audio output, so that is
    // dropped once and then remembered.
    let mut want_audio = audio_port.is_some();

    while !stop.load(Ordering::Relaxed) {
        publish_status(slot, Status::Connecting, None);

        let mut builder = FfmpegCommand::new();
        builder
            .hide_banner()
            // TCP survives packet loss on wifi better than UDP.
            .args(["-rtsp_transport", "tcp"])
            // Bound a stalled socket so the thread cannot block forever.
            .args(["-timeout", IO_TIMEOUT_US])
            // Latency: stop every stage from holding on to frames.
            .args(["-fflags", "nobuffer", "-flags", "low_delay"])
            // Frame threading hands back nothing until every thread holds a
            // picture, so a many-core machine waits before the first frame
            // arrives and stays that far behind. Measured on a 1080p stream:
            // one thread is 0.22s closer to live than the default. Adding
            // -avioflags direct, -max_delay 0 and -reorder_queue_size 0 gave
            // that back, so they are not here.
            .args(["-threads", "1"])
            .input(url)
            // Picture to standard output.
            .args([
                "-map",
                "0:v",
                "-an",
                "-vf",
                &picture_filter(),
                // Every decoded picture, in order. The default re-times the
                // stream to its nominal frame rate, and these cameras skip
                // pictures and jump their timestamps; re-timing then drops
                // some pictures and holds the rest, which froze the wall for
                // half a second at a time.
                "-fps_mode",
                "passthrough",
                "-f",
                "rawvideo",
            ])
            // GPUI uploads frames as BGRA, so let ffmpeg convert for us.
            .args(["-pix_fmt", "bgra", "-"]);

        // The camera's sound comes off the same process, down a loopback
        // socket. One camera, one connection: many cameras allow only two or
        // three, and a separate audio process is often refused.
        if want_audio && let Some(port) = audio_port {
            builder.args([
                "-map",
                "0:a?",
                "-vn",
                "-f",
                "f32le",
                "-ar",
                "48000",
                "-ac",
                "2",
                &format!("tcp://127.0.0.1:{port}"),
            ]);
        }

        let mut child = match builder.spawn() {
            Ok(child) => child,
            Err(err) => {
                publish_status(
                    slot,
                    Status::Error(format!("cannot run ffmpeg: {err}")),
                    None,
                );
                return;
            }
        };

        // The picture is read straight off the pipe. With two outputs
        // ffmpeg-sidecar cannot tell which output standard output carries and
        // hands back no frames at all.
        let size: Arc<Mutex<Option<(u32, u32)>>> = Arc::new(Mutex::new(None));
        if let Some(stdout) = child.take_stdout() {
            spawn_frame_reader(stdout, size.clone(), slot.clone(), stop.clone());
        }

        let mut last_error = None;
        let mut live = false;

        let iter = child.iter();
        match iter {
            Ok(iter) => {
                for event in iter {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    match event {
                        // The camera's own picture, before it was decoded, is
                        // the codec the sheet prints as `H264` or `HEVC`.
                        FfmpegEvent::ParsedInputStream(stream) => {
                            if stream.is_video() && !stream.format.is_empty() {
                                publish_codec(slot, stream.format.clone());
                            }
                        }
                        // The raw output is the only bgra stream in the log,
                        // which is how it is told apart from the input.
                        FfmpegEvent::ParsedOutputStream(stream) => {
                            if let Some(video) = stream.video_data()
                                && video.pix_fmt.contains("bgra")
                            {
                                *size.lock().expect("frame size") =
                                    Some((video.width, video.height));
                                if !live {
                                    live = true;
                                    publish_status(
                                        slot,
                                        Status::Live,
                                        Some((video.width, video.height)),
                                    );
                                }
                            }
                        }
                        FfmpegEvent::Log(LogLevel::Error | LogLevel::Fatal, message) => {
                            // A stream without sound makes an audio output
                            // impossible; the picture is fine either way.
                            if message.contains("does not contain any stream") {
                                want_audio = false;
                            }
                            last_error = Some(message);
                        }
                        FfmpegEvent::Error(message) => last_error = Some(message),
                        _ => {}
                    }
                }
            }
            Err(err) => last_error = Some(err.to_string()),
        }

        // Kill the child: dropping it alone leaves the process running.
        let _ = child.kill();
        let _ = child.wait();

        if stop.load(Ordering::Relaxed) {
            return;
        }

        let message = last_error.unwrap_or_else(|| "stream ended".to_string());
        publish_status(slot, Status::Error(message), None);

        let mut waited = Duration::ZERO;
        while waited < RECONNECT_DELAY {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(RECONNECT_POLL);
            waited += RECONNECT_POLL;
        }
    }
}

/// Reads raw frames off the child's standard output, one picture at a time.
///
/// The size comes from the ffmpeg log, so the first read waits for it.
fn spawn_frame_reader(
    mut stdout: std::process::ChildStdout,
    size: Arc<Mutex<Option<(u32, u32)>>>,
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
) {
    let _ = thread::Builder::new()
        .name("camviewer-frames".to_string())
        .spawn(move || {
            let mut frame = Vec::new();
            let mut rate = Rate::new();

            while !stop.load(Ordering::Relaxed) {
                let Some((width, height)) = *size.lock().expect("frame size") else {
                    thread::sleep(RECONNECT_POLL);
                    continue;
                };

                let expected = width as usize * height as usize * 4;
                if frame.len() != expected {
                    frame.resize(expected, 0);
                }

                if std::io::Read::read_exact(&mut stdout, &mut frame).is_err() {
                    // ffmpeg closed the pipe; the outer loop reconnects.
                    return;
                }

                let fps = rate.note(Instant::now());
                publish_frame(&slot, width, height, &frame, fps);
            }
        });
}

/// Counts decoded pictures over a fixed window, so the sheet prints a rate the
/// camera is really delivering rather than the frame rate it claims.
///
/// The cameras skip pictures and jump their timestamps, so the nominal rate in
/// the ffmpeg log is not the rate the wall receives.
struct Rate {
    /// When the window opened. `None` until the first picture.
    started: Option<Instant>,
    /// Pictures counted in the open window.
    frames: u32,
}

impl Rate {
    fn new() -> Self {
        Self {
            started: None,
            frames: 0,
        }
    }

    /// Records one picture and returns the rate once a window is complete.
    ///
    /// The picture that completes a window opens the next one, so no picture
    /// is counted in two windows.
    fn note(&mut self, now: Instant) -> Option<f32> {
        let started = *self.started.get_or_insert(now);
        let elapsed = now.saturating_duration_since(started);

        if elapsed < RATE_WINDOW {
            self.frames += 1;
            return None;
        }

        let fps = self.frames as f32 / elapsed.as_secs_f32();
        self.started = Some(now);
        self.frames = 1;
        Some(fps)
    }
}

/// Scale down to [`MAX_WIDTH`], never up. The escaped comma separates the
/// width and height arguments of the `scale` filter.
fn picture_filter() -> String {
    // `setpts` renumbers the timestamps to 0, 1, 2, ... at the nominal rate.
    // The cameras repeat timestamps and jump backwards, and the rawvideo muxer
    // drops any picture whose timestamp does not advance. The reader throws the
    // timestamps away, so the only thing that matters is that every picture
    // reaches it, once, in order.
    format!("scale=w=min({MAX_WIDTH}\\,iw):h=-2,setpts=N/FRAME_RATE/TB")
}

fn publish_status(slot: &Arc<Mutex<Slot>>, status: Status, resolution: Option<(u32, u32)>) {
    if let Ok(mut slot) = slot.lock() {
        // A connection that is not live has no current rate or codec. A live
        // one keeps both: the codec arrives before the status does, and the
        // rate arrives with the frames after it.
        if !matches!(status, Status::Live) {
            slot.fps = None;
            slot.codec = None;
        }
        slot.status = status;
        slot.resolution = resolution;
        slot.seq += 1;
    }
}

fn publish_codec(slot: &Arc<Mutex<Slot>>, codec: String) {
    if let Ok(mut slot) = slot.lock()
        && slot.codec.as_deref() != Some(codec.as_str())
    {
        slot.codec = Some(codec);
        slot.seq += 1;
    }
}

fn publish_frame(slot: &Arc<Mutex<Slot>>, width: u32, height: u32, data: &[u8], fps: Option<f32>) {
    // The reader reuses its buffer, so the pixels are copied once here.
    let Some(buffer) = RgbaImage::from_raw(width, height, data.to_vec()) else {
        return;
    };

    // The buffer holds BGRA bytes in an RGBA-typed buffer on purpose: that is
    // the byte order GPUI uploads, and it avoids a per-pixel swap.
    let image = Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)));

    if let Ok(mut slot) = slot.lock() {
        slot.frame = Some(image);
        slot.resolution = Some((width, height));
        if fps.is_some() {
            slot.fps = fps;
        }
        slot.seq += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_WIDTH, Rate, Slot, Status, picture_filter, publish_status};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[test]
    fn the_picture_filter_caps_the_width_and_keeps_the_shape() {
        let filter = picture_filter();

        // The comma inside min() has to be escaped, or the filter parser reads
        // it as the end of the width argument.
        assert!(
            filter.contains(&format!("min({MAX_WIDTH}\\,iw)")),
            "{filter}"
        );
        // -2 keeps the height even, which some encoders and the BGRA upload want.
        assert!(filter.contains(":h=-2,"), "{filter}");
        assert!(filter.starts_with("scale=w="), "{filter}");
        assert!(
            !filter.contains(" "),
            "a space would split the argument: {filter}"
        );
    }

    #[test]
    fn the_picture_filter_numbers_the_timestamps_in_order() {
        let filter = picture_filter();

        // The cameras jump their timestamps backwards, and a muxer fed a
        // timestamp that does not advance drops the picture. setpts replaces
        // them with the frame's own index, which always advances.
        assert!(filter.ends_with(",setpts=N/FRAME_RATE/TB"), "{filter}");
    }

    #[test]
    fn the_rate_meter_reports_the_pictures_a_second_really_carries() {
        let start = Instant::now();
        let mut rate = Rate::new();

        // 25 pictures in one second, one every 40 ms.
        for step in 0..25 {
            let now = start + Duration::from_millis(step * 40);
            assert_eq!(rate.note(now), None, "a window is not complete at {step}");
        }

        // The picture at exactly one second opens the next window.
        let fps = rate
            .note(start + Duration::from_secs(1))
            .expect("one second of pictures should print a rate");
        assert!((fps - 25.0).abs() < 0.01, "expected 25 fps, got {fps}");
    }

    #[test]
    fn the_rate_meter_counts_a_gap_as_a_slow_rate() {
        let start = Instant::now();
        let mut rate = Rate::new();

        assert_eq!(rate.note(start), None);
        let fps = rate
            .note(start + Duration::from_secs(2))
            .expect("two seconds should complete a window");

        // One picture arrived after the first, two seconds later. The rate
        // says so rather than repeating the rate from before the gap.
        assert!((fps - 0.5).abs() < 0.01, "expected 0.5 fps, got {fps}");
    }

    #[test]
    fn a_new_connection_forgets_the_old_rate_and_codec() {
        let slot = Arc::new(Mutex::new(Slot::default()));
        {
            let mut slot = slot.lock().expect("slot");
            slot.fps = Some(25.0);
            slot.codec = Some("h264".to_string());
        }

        // A live connection keeps what the decoder just reported.
        publish_status(&slot, Status::Live, Some((1280, 720)));
        {
            let slot = slot.lock().expect("slot");
            assert_eq!(slot.fps, Some(25.0));
            assert_eq!(slot.codec.as_deref(), Some("h264"));
        }

        // A connection that ends, or is starting again, has neither.
        publish_status(&slot, Status::Connecting, None);
        let slot = slot.lock().expect("slot");
        assert_eq!(slot.fps, None);
        assert_eq!(slot.codec, None);
    }
}
