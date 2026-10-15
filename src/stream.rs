//! Video decoding.
//!
//! One background thread runs one `ffmpeg` process per camera. The thread
//! decodes the RTSP stream into BGRA frames and publishes the newest frame into
//! a shared slot. The UI reads the slot on a timer, so a slow repaint drops
//! frames instead of queueing them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel, OutputVideoFrame};
use gpui_kit::RenderImage;
use image::{Frame, RgbaImage};
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
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            seq: 0,
            frame: None,
            status: Status::Connecting,
            resolution: None,
        }
    }
}

/// A running video decoder. Dropping the handle stops the decoder thread.
pub struct VideoStream {
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
}

impl VideoStream {
    pub fn start(url: String) -> Self {
        let slot = Arc::new(Mutex::new(Slot::default()));
        let stop = Arc::new(AtomicBool::new(false));

        let thread_slot = slot.clone();
        let thread_stop = stop.clone();
        let _ = thread::Builder::new()
            .name("camviewer-video".to_string())
            .spawn(move || decode_loop(&url, &thread_slot, &thread_stop));

        Self { slot, stop }
    }

    /// The shared slot that the decoder thread writes into.
    pub fn slot(&self) -> Arc<Mutex<Slot>> {
        self.slot.clone()
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        // The thread kills its own ffmpeg child on its next frame.
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn decode_loop(url: &str, slot: &Arc<Mutex<Slot>>, stop: &Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        publish_status(slot, Status::Connecting, None);

        let mut child = match FfmpegCommand::new()
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
            .args(["-an", "-vf", &scale_filter(), "-f", "rawvideo"])
            // GPUI uploads frames as BGRA, so let ffmpeg convert for us.
            .args(["-pix_fmt", "bgra", "-"])
            .spawn()
        {
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
                        FfmpegEvent::OutputFrame(frame) => {
                            if !live {
                                live = true;
                                publish_status(
                                    slot,
                                    Status::Live,
                                    Some((frame.width, frame.height)),
                                );
                            }
                            publish_frame(slot, frame);
                        }
                        FfmpegEvent::Log(LogLevel::Error | LogLevel::Fatal, message) => {
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

/// Scale down to [`MAX_WIDTH`], never up. The escaped comma separates the
/// width and height arguments of the `scale` filter.
fn scale_filter() -> String {
    format!("scale=w=min({MAX_WIDTH}\\,iw):h=-2")
}

fn publish_status(slot: &Arc<Mutex<Slot>>, status: Status, resolution: Option<(u32, u32)>) {
    if let Ok(mut slot) = slot.lock() {
        slot.status = status;
        slot.resolution = resolution;
        slot.seq += 1;
    }
}

fn publish_frame(slot: &Arc<Mutex<Slot>>, frame: OutputVideoFrame) {
    let OutputVideoFrame {
        width,
        height,
        data,
        ..
    } = frame;

    // `data` is moved into the image, so no pixel copy happens here.
    let Some(buffer) = RgbaImage::from_raw(width, height, data) else {
        return;
    };

    // The buffer holds BGRA bytes in an RGBA-typed buffer on purpose: that is
    // the byte order GPUI uploads, and it avoids a per-pixel swap.
    let image = Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)));

    if let Ok(mut slot) = slot.lock() {
        slot.frame = Some(image);
        slot.resolution = Some((width, height));
        slot.seq += 1;
    }
}
