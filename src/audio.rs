//! Audio playback.
//!
//! A second `ffmpeg` process per camera decodes the audio track to raw 32-bit
//! float PCM. A dedicated thread pushes that PCM into a `rodio` queue.
//!
//! Audio starts only when a camera is unmuted, so a muted wall of cameras
//! spends no CPU, no bandwidth and no camera session on sound.

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use ffmpeg_sidecar::command::FfmpegCommand;
use rodio::buffer::SamplesBuffer;
use rodio::queue::{SourcesQueueInput, SourcesQueueOutput};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, nz};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
/// Read audio in 100 ms blocks.
const BLOCK_SAMPLES: usize = SAMPLE_RATE as usize / 10;

/// The process-wide output device. Create one and keep it alive.
pub struct AudioEngine {
    sink: MixerDeviceSink,
}

impl AudioEngine {
    /// Opens the default output device. Returns `None` when the machine has no
    /// usable audio device, in which case video still works.
    pub fn new() -> Option<Self> {
        DeviceSinkBuilder::open_default_sink()
            .ok()
            .map(|sink| Self { sink })
    }

    /// Starts playback for one RTSP URL. Call [`AudioHandle`]'s drop to stop.
    pub fn play(&self, url: &str) -> AudioHandle {
        let (queue_input, queue_output): (Arc<SourcesQueueInput>, SourcesQueueOutput) =
            rodio::queue::queue(true);

        let player = Player::connect_new(self.sink.mixer());
        player.append(queue_output);

        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_url = url.to_string();
        let _ = thread::Builder::new()
            .name("camviewer-audio".to_string())
            .spawn(move || audio_loop(&thread_url, &thread_stop, &queue_input));

        AudioHandle {
            stop,
            _player: player,
        }
    }
}

/// A playing audio stream. Dropping it stops the sound and the ffmpeg process.
pub struct AudioHandle {
    stop: Arc<AtomicBool>,
    _player: Player,
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn audio_loop(url: &str, stop: &Arc<AtomicBool>, queue_input: &Arc<SourcesQueueInput>) {
    let mut child = match FfmpegCommand::new()
        .hide_banner()
        // Nothing is parsed from the log here, so silence it.
        .args(["-loglevel", "error"])
        .args(["-rtsp_transport", "tcp"])
        .args(["-timeout", "5000000"])
        .input(url)
        .args(["-vn", "-f", "f32le", "-ar", "48000", "-ac", "2", "-"])
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return,
    };

    let Some(mut stdout) = child.take_stdout() else {
        let _ = child.kill();
        return;
    };

    let mut bytes = vec![0u8; BLOCK_SAMPLES * CHANNELS as usize * size_of::<f32>()];

    while !stop.load(Ordering::Relaxed) {
        if stdout.read_exact(&mut bytes).is_err() {
            break;
        }

        let (blocks, _) = bytes.as_chunks::<4>();
        let samples: Vec<f32> = blocks.iter().map(|raw| f32::from_le_bytes(*raw)).collect();

        queue_input.append(SamplesBuffer::new(nz!(2), nz!(48000), samples));
    }

    let _ = child.kill();
    let _ = child.wait();
}
