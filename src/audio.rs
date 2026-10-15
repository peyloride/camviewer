//! Audio playback.
//!
//! The camera's audio arrives on the same connection as its video: the video
//! `ffmpeg` writes the picture to its standard output and the sound to a
//! loopback socket. A camera therefore needs one connection, not two, which
//! matters because many cameras allow only two or three.
//!
//! The socket is read all the time. Samples go to a player when one is
//! attached and are dropped otherwise, so `ffmpeg` is never left waiting on a
//! socket nobody reads, which would stall the picture as well.

use std::io::{ErrorKind, Read};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::queue::SourcesQueueInput;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, nz};

/// What the video `ffmpeg` is told to produce, and what the player expects.
pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;

/// Audio arrives in 100 ms blocks.
const BLOCK_SAMPLES: usize = SAMPLE_RATE as usize / 10;
const BLOCK_BYTES: usize = BLOCK_SAMPLES * CHANNELS as usize * size_of::<f32>();
/// How long a blocked reader waits before it looks at the stop flag again.
const READ_TIMEOUT: Duration = Duration::from_millis(200);
/// How long an idle listener waits before it looks at the stop flag again.
const ACCEPT_POLL: Duration = Duration::from_millis(50);

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

    /// Starts sound for one camera. Drop the handle to stop it.
    pub fn play(&self, pipe: &PcmPipe) -> AudioHandle {
        let (queue_input, queue_output) = rodio::queue::queue(true);

        let player = Player::connect_new(self.sink.mixer());
        player.append(queue_output);
        pipe.attach(queue_input);

        AudioHandle {
            pipe: pipe.clone(),
            _player: player,
        }
    }
}

/// One camera's sound. Dropping it detaches the player and stops it.
pub struct AudioHandle {
    pipe: PcmPipe,
    _player: Player,
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.pipe.detach();
    }
}

/// PCM read from a camera, waiting for something to play it.
#[derive(Clone, Default)]
pub struct PcmPipe {
    queue: Arc<Mutex<Option<Arc<SourcesQueueInput>>>>,
}

impl PcmPipe {
    fn attach(&self, queue: Arc<SourcesQueueInput>) {
        if let Ok(mut slot) = self.queue.lock() {
            *slot = Some(queue);
        }
    }

    fn detach(&self) {
        if let Ok(mut slot) = self.queue.lock() {
            *slot = None;
        }
    }

    /// The attached player, if any. Nothing is queued when there is none.
    fn player(&self) -> Option<Arc<SourcesQueueInput>> {
        self.queue.lock().ok().and_then(|slot| slot.clone())
    }
}

/// Reads the camera's audio socket until `stop` is set. Returns the handle a
/// player attaches to.
pub fn drain(listener: TcpListener, stop: Arc<AtomicBool>) -> PcmPipe {
    let pipe = PcmPipe::default();
    let reader = pipe.clone();

    // A non-blocking listener lets the thread notice `stop` while idle.
    listener.set_nonblocking(true).ok();

    let _ = thread::Builder::new()
        .name("camviewer-audio".to_string())
        .spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => read_connection(stream, &reader, &stop),
                    Err(err) if err.kind() == ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
                    Err(_) => break,
                }
            }
        });

    pipe
}

/// Reads one `ffmpeg` connection to its end, then returns so the caller waits
/// for the next one. A reconnect opens a new socket.
fn read_connection(mut stream: TcpStream, pipe: &PcmPipe, stop: &Arc<AtomicBool>) {
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok();

    let mut bytes = vec![0u8; BLOCK_BYTES];
    let mut filled = 0;

    while !stop.load(Ordering::Relaxed) {
        match stream.read(&mut bytes[filled..]) {
            Ok(0) => return,
            Ok(read) => {
                filled += read;
                if filled < BLOCK_BYTES {
                    continue;
                }
                filled = 0;

                if let Some(player) = pipe.player() {
                    let (blocks, _) = bytes.as_chunks::<4>();
                    let samples: Vec<f32> =
                        blocks.iter().map(|raw| f32::from_le_bytes(*raw)).collect();
                    player.append(SamplesBuffer::new(nz!(2), nz!(48000), samples));
                }
                // With no player attached the block is dropped on the floor:
                // reading it is what keeps ffmpeg from blocking.
            }
            Err(err)
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut => {}
            Err(_) => return,
        }
    }
}
