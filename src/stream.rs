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
use std::time::Duration;

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

                publish_frame(&slot, width, height, &frame);
            }
        });
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
        slot.status = status;
        slot.resolution = resolution;
        slot.seq += 1;
    }
}

fn publish_frame(slot: &Arc<Mutex<Slot>>, width: u32, height: u32, data: &[u8]) {
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
        slot.seq += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_WIDTH, picture_filter};

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
}
