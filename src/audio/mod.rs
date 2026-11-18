//! Audio output: AAudio on Android 8.1+, OpenSL ES on older versions (and as a fallback).
//!
//! Both libraries are loaded at runtime (see `dl`) instead of being linked, so that this
//! library loads on every supported Android version, even where AAudio doesn't exist.

mod aaudio;
mod dl;
mod opensles;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ruffle_core::backend::audio::{
    swf, AudioBackend, AudioMixer, AudioMixerProxy, DecodeError, RegisterError, SoundHandle,
    SoundInstanceHandle, SoundStreamInfo, SoundTransform,
};
use ruffle_core::impl_audio_mixer_backend;

use self::aaudio::AAudioOutput;
use self::opensles::OpenSlesOutput;

/// The sample rate of the mixer, and of the output stream. AAudio and OpenSL ES resample
/// it to the rate of the device if needed.
const SAMPLE_RATE: u32 = 44100;

/// Like Oboe, only use AAudio on Android 8.1 (API 27) and above, because it has bugs on 8.0.
const AAUDIO_MIN_SDK_VERSION: i32 = 27;

/// How long to wait before retrying to open an output after failing to reopen one.
/// Doubled after each failed attempt, up to `MAX_RETRY_DELAY`.
const MIN_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// Mixes the currently playing sounds into `buffer` (interleaved stereo).
///
/// This runs on an audio thread of the OS, so it must not unwind. The mixer only panics if
/// one of its locks got poisoned (by a panic on another thread); then this plays silence.
fn mix(proxy: &AudioMixerProxy, buffer: &mut [f32], panicked: &AtomicBool) {
    if !panicked.load(Ordering::Relaxed) {
        if catch_unwind(AssertUnwindSafe(|| proxy.mix::<f32>(buffer))).is_ok() {
            return;
        }
        // Don't keep panicking (and logging the panic) on every callback.
        panicked.store(true, Ordering::Relaxed);
    }
    buffer.fill(0.0);
}

enum Output {
    AAudio(AAudioOutput),
    OpenSles(OpenSlesOutput),
}

impl Output {
    fn name(&self) -> &'static str {
        match self {
            Output::AAudio(_) => "AAudio",
            Output::OpenSles(_) => "OpenSL ES",
        }
    }

    fn start(&mut self) -> Result<(), String> {
        match self {
            Output::AAudio(output) => output.start(),
            Output::OpenSles(output) => output.start(),
        }
    }

    fn pause(&mut self) -> Result<(), String> {
        match self {
            Output::AAudio(output) => output.pause(),
            Output::OpenSles(output) => output.pause(),
        }
    }

    fn restart_reason(&self) -> Option<String> {
        match self {
            Output::AAudio(output) => output.restart_reason(),
            Output::OpenSles(output) => output.restart_reason(),
        }
    }
}

pub struct AndroidAudioBackend {
    /// `None` only if reopening the output failed; then it's retried later.
    /// (Declared before `mixer`, so it's closed first when dropped.)
    output: Option<Output>,
    mixer: AudioMixer,
    paused: bool,
    sdk_version: i32,
    /// Whether to try AAudio (before OpenSL ES) when opening an output.
    try_aaudio: bool,
    next_retry: Instant,
    retry_delay: Duration,
}

impl AndroidAudioBackend {
    /// `sdk_version` is the API level of the device.
    pub fn new(sdk_version: i32) -> Result<Self, String> {
        let mut backend = Self {
            output: None,
            mixer: AudioMixer::new(2, SAMPLE_RATE),
            paused: true,
            sdk_version,
            try_aaudio: sdk_version >= AAUDIO_MIN_SDK_VERSION,
            next_retry: Instant::now(),
            retry_delay: MIN_RETRY_DELAY,
        };
        backend.output = Some(backend.open_output()?);
        Ok(backend)
    }

    /// Opens a (paused) output: with AAudio if it should be used and it works,
    /// otherwise with OpenSL ES.
    fn open_output(&mut self) -> Result<Output, String> {
        let mut aaudio_error = None;
        if self.try_aaudio {
            match AAudioOutput::open(self.mixer.proxy(), SAMPLE_RATE, self.sdk_version) {
                Ok(output) => {
                    log::info!("Playing audio with AAudio");
                    return Ok(Output::AAudio(output));
                }
                Err(e) => {
                    log::warn!("Couldn't open AAudio output, trying OpenSL ES instead: {e}");
                    // Don't try it again: it's unlikely to start working later.
                    self.try_aaudio = false;
                    aaudio_error = Some(e);
                }
            }
        }
        match OpenSlesOutput::open(self.mixer.proxy(), SAMPLE_RATE) {
            Ok(output) => {
                log::info!("Playing audio with OpenSL ES");
                Ok(Output::OpenSles(output))
            }
            Err(e) => Err(match aaudio_error {
                Some(aaudio_error) => format!("AAudio: {aaudio_error}; OpenSL ES: {e}"),
                None => format!("OpenSL ES: {e}"),
            }),
        }
    }

    /// Reopens the output if it broke (for example, AAudio streams have to be reopened
    /// when headphones are plugged in or out). Called every frame, cheap if nothing is wrong.
    pub fn recreate_stream_if_needed(&mut self) {
        match &self.output {
            Some(output) => match output.restart_reason() {
                Some(reason) => log::warn!("Reopening {} audio output: {reason}", output.name()),
                None => return,
            },
            None => {
                if Instant::now() < self.next_retry {
                    return;
                }
            }
        }

        // Close the old stream before opening the new one.
        self.output = None;
        match self.open_output() {
            Ok(mut output) => {
                if !self.paused {
                    if let Err(e) = output.start() {
                        log::error!("Couldn't start {} audio output: {e}", output.name());
                    }
                }
                self.output = Some(output);
                self.retry_delay = MIN_RETRY_DELAY;
            }
            Err(e) => {
                log::error!(
                    "Couldn't reopen audio output, retrying in {:?}: {e}",
                    self.retry_delay
                );
                self.next_retry = Instant::now() + self.retry_delay;
                self.retry_delay = (self.retry_delay * 2).min(MAX_RETRY_DELAY);
            }
        }
    }
}

impl AudioBackend for AndroidAudioBackend {
    impl_audio_mixer_backend!(mixer);

    fn play(&mut self) {
        self.paused = false;
        if let Some(output) = &mut self.output {
            if let Err(e) = output.start() {
                log::error!("Couldn't start {} audio output: {e}", output.name());
            }
        }
    }

    fn pause(&mut self) {
        self.paused = true;
        if let Some(output) = &mut self.output {
            if let Err(e) = output.pause() {
                log::error!("Couldn't pause {} audio output: {e}", output.name());
            }
        }
    }
}
