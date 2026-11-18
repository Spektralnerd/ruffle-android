//! Audio output through AAudio (Android 8.0+), with `libaaudio.so` loaded at runtime.
//!
//! The prototypes and constants are from the NDK's `aaudio/AAudio.h` (as bound by
//! `ndk-sys` 0.6.0), and were cross-checked against Oboe (`src/aaudio/AAudioLoader.{h,cpp}`
//! and `include/oboe/Definitions.h`). The usage follows Oboe's `AudioStreamAAudio`.

use std::ffi::{c_char, c_void, CStr};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use ruffle_core::backend::audio::AudioMixerProxy;

use super::dl;

/// Opaque `AAudioStream`.
#[repr(C)]
struct AAudioStream {
    _private: [u8; 0],
}

/// Opaque `AAudioStreamBuilder`.
#[repr(C)]
struct AAudioStreamBuilder {
    _private: [u8; 0],
}

// All of these are `int32_t` in the C API.
const AAUDIO_OK: i32 = 0;
const AAUDIO_DIRECTION_OUTPUT: i32 = 0;
const AAUDIO_FORMAT_PCM_FLOAT: i32 = 2;
const AAUDIO_PERFORMANCE_MODE_LOW_LATENCY: i32 = 12;
/// Only meaningful with `AAudioStreamBuilder_setUsage` (API 28+).
const AAUDIO_USAGE_GAME: i32 = 14;
const AAUDIO_CALLBACK_RESULT_CONTINUE: i32 = 0;
const AAUDIO_STREAM_STATE_STARTING: i32 = 3;
const AAUDIO_STREAM_STATE_STARTED: i32 = 4;
const AAUDIO_STREAM_STATE_PAUSING: i32 = 5;
const AAUDIO_STREAM_STATE_PAUSED: i32 = 6;
const AAUDIO_STREAM_STATE_DISCONNECTED: i32 = 13;

const CHANNEL_COUNT: i32 = 2;

/// `AAudioStream_dataCallback`
type DataCallback = unsafe extern "C" fn(
    stream: *mut AAudioStream,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    num_frames: i32,
) -> i32;
/// `AAudioStream_errorCallback`
type ErrorCallback =
    unsafe extern "C" fn(stream: *mut AAudioStream, user_data: *mut c_void, error: i32);

/// `aaudio_result_t AAudio_createStreamBuilder(AAudioStreamBuilder **builder)`
type CreateStreamBuilderFn = unsafe extern "C" fn(builder: *mut *mut AAudioStreamBuilder) -> i32;
/// `void AAudioStreamBuilder_setX(AAudioStreamBuilder *builder, int32_t value)`, for
/// `setDirection`, `setFormat`, `setChannelCount`, `setSampleRate`,
/// `setPerformanceMode` and `setUsage`.
type BuilderSetFn = unsafe extern "C" fn(builder: *mut AAudioStreamBuilder, value: i32);
/// `void AAudioStreamBuilder_setDataCallback(AAudioStreamBuilder *builder,
///     AAudioStream_dataCallback callback, void *userData)`
type BuilderSetDataCallbackFn = unsafe extern "C" fn(
    builder: *mut AAudioStreamBuilder,
    callback: Option<DataCallback>,
    user_data: *mut c_void,
);
/// `void AAudioStreamBuilder_setErrorCallback(AAudioStreamBuilder *builder,
///     AAudioStream_errorCallback callback, void *userData)`
type BuilderSetErrorCallbackFn = unsafe extern "C" fn(
    builder: *mut AAudioStreamBuilder,
    callback: Option<ErrorCallback>,
    user_data: *mut c_void,
);
/// `aaudio_result_t AAudioStreamBuilder_openStream(AAudioStreamBuilder *builder,
///     AAudioStream **stream)`
type BuilderOpenStreamFn =
    unsafe extern "C" fn(builder: *mut AAudioStreamBuilder, stream: *mut *mut AAudioStream) -> i32;
/// `aaudio_result_t AAudioStreamBuilder_delete(AAudioStreamBuilder *builder)`
type BuilderDeleteFn = unsafe extern "C" fn(builder: *mut AAudioStreamBuilder) -> i32;
/// `int32_t AAudioStream_x(AAudioStream *stream)`, for `requestStart`, `requestPause`,
/// `requestStop`, `close` (all returning `aaudio_result_t`), `getState`
/// (`aaudio_stream_state_t`), `getSampleRate`, `getChannelCount`, `getFormat`
/// (`aaudio_format_t`) and `getFramesPerBurst`.
type StreamFn = unsafe extern "C" fn(stream: *mut AAudioStream) -> i32;
/// `const char *AAudio_convertResultToText(aaudio_result_t returnCode)`
type ConvertResultToTextFn = unsafe extern "C" fn(result: i32) -> *const c_char;

/// The AAudio functions, loaded from `libaaudio.so`.
struct AAudio {
    create_stream_builder: CreateStreamBuilderFn,
    builder_set_direction: BuilderSetFn,
    builder_set_format: BuilderSetFn,
    builder_set_channel_count: BuilderSetFn,
    builder_set_sample_rate: BuilderSetFn,
    builder_set_performance_mode: BuilderSetFn,
    /// API 28+.
    builder_set_usage: Option<BuilderSetFn>,
    builder_set_data_callback: BuilderSetDataCallbackFn,
    builder_set_error_callback: BuilderSetErrorCallbackFn,
    builder_open_stream: BuilderOpenStreamFn,
    builder_delete: BuilderDeleteFn,
    stream_request_start: StreamFn,
    stream_request_pause: StreamFn,
    stream_request_stop: StreamFn,
    stream_close: StreamFn,
    stream_get_state: StreamFn,
    stream_get_sample_rate: StreamFn,
    stream_get_channel_count: StreamFn,
    stream_get_format: StreamFn,
    stream_get_frames_per_burst: Option<StreamFn>,
    convert_result_to_text: Option<ConvertResultToTextFn>,
}

impl AAudio {
    /// Loads `libaaudio.so` once per process.
    fn get(sdk_version: i32) -> Result<&'static Self, String> {
        static AAUDIO: OnceLock<Result<AAudio, String>> = OnceLock::new();
        AAUDIO
            .get_or_init(|| {
                // SAFETY: The function pointer types match the prototypes in AAudio.h.
                unsafe { Self::load(sdk_version) }
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    unsafe fn load(sdk_version: i32) -> Result<Self, String> {
        let lib = dl::Library::open("libaaudio.so\0")?;
        Ok(Self {
            create_stream_builder: lib.function("AAudio_createStreamBuilder\0")?,
            builder_set_direction: lib.function("AAudioStreamBuilder_setDirection\0")?,
            builder_set_format: lib.function("AAudioStreamBuilder_setFormat\0")?,
            // Like Oboe, fall back to the old (deprecated) name if needed.
            builder_set_channel_count: lib
                .function("AAudioStreamBuilder_setChannelCount\0")
                .or_else(|_| lib.function("AAudioStreamBuilder_setSamplesPerFrame\0"))?,
            builder_set_sample_rate: lib.function("AAudioStreamBuilder_setSampleRate\0")?,
            builder_set_performance_mode: lib
                .function("AAudioStreamBuilder_setPerformanceMode\0")?,
            // Like Oboe, only look this up where it's documented to exist.
            builder_set_usage: if sdk_version >= 28 {
                lib.function("AAudioStreamBuilder_setUsage\0").ok()
            } else {
                None
            },
            builder_set_data_callback: lib.function("AAudioStreamBuilder_setDataCallback\0")?,
            builder_set_error_callback: lib.function("AAudioStreamBuilder_setErrorCallback\0")?,
            builder_open_stream: lib.function("AAudioStreamBuilder_openStream\0")?,
            builder_delete: lib.function("AAudioStreamBuilder_delete\0")?,
            stream_request_start: lib.function("AAudioStream_requestStart\0")?,
            stream_request_pause: lib.function("AAudioStream_requestPause\0")?,
            stream_request_stop: lib.function("AAudioStream_requestStop\0")?,
            stream_close: lib.function("AAudioStream_close\0")?,
            stream_get_state: lib.function("AAudioStream_getState\0")?,
            stream_get_sample_rate: lib.function("AAudioStream_getSampleRate\0")?,
            stream_get_channel_count: lib
                .function("AAudioStream_getChannelCount\0")
                .or_else(|_| lib.function("AAudioStream_getSamplesPerFrame\0"))?,
            stream_get_format: lib.function("AAudioStream_getFormat\0")?,
            stream_get_frames_per_burst: lib.function("AAudioStream_getFramesPerBurst\0").ok(),
            convert_result_to_text: lib.function("AAudio_convertResultToText\0").ok(),
        })
    }

    fn result_text(&self, result: i32) -> String {
        if let Some(convert) = self.convert_result_to_text {
            // SAFETY: This returns a pointer to a static C string, or null.
            let text = unsafe { convert(result) };
            if !text.is_null() {
                // SAFETY: Checked for null above.
                let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
                return format!("{text} ({result})");
            }
        }
        format!("error {result}")
    }

    fn check(&self, result: i32, what: &str) -> Result<(), String> {
        if result == AAUDIO_OK {
            Ok(())
        } else {
            Err(format!("{what} failed: {}", self.result_text(result)))
        }
    }
}

/// The data shared with the data callback. It's only ever accessed through
/// shared references, and it's freed only after the stream is closed.
struct CallbackData {
    proxy: AudioMixerProxy,
    panicked: AtomicBool,
}

/// The stream for which the error callback was called most recently, and the error.
///
/// The error callback only records this, and doesn't touch any per-stream data, because
/// AAudio doesn't clearly guarantee that an error callback can't still be running (on
/// another thread) when `AAudioStream_close` returns. (Oboe guards against that case too.)
/// Only one stream is open at a time, and a stale value can at worst cause one
/// unnecessary reopening of the stream.
static FAILED_STREAM: AtomicPtr<AAudioStream> = AtomicPtr::new(ptr::null_mut());
static FAILED_STREAM_ERROR: AtomicI32 = AtomicI32::new(AAUDIO_OK);

/// Called by AAudio on its own high-priority thread whenever it needs more audio data.
unsafe extern "C" fn data_callback(
    _stream: *mut AAudioStream,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    num_frames: i32,
) -> i32 {
    if user_data.is_null() || audio_data.is_null() || num_frames <= 0 {
        return AAUDIO_CALLBACK_RESULT_CONTINUE;
    }
    // SAFETY: `user_data` is the `CallbackData` of this stream, which outlives the stream.
    let data = &*(user_data as *const CallbackData);
    // SAFETY: The stream was checked to be 2-channel float, so AAudio passes a buffer
    // of `num_frames * 2` `f32` samples. `num_frames` is positive (checked above).
    let buffer = std::slice::from_raw_parts_mut(
        audio_data as *mut f32,
        num_frames as usize * CHANNEL_COUNT as usize,
    );
    super::mix(&data.proxy, buffer, &data.panicked);
    // Never return STOP: that "caused various problems before S" (Oboe), and
    // stopping and pausing is done from the main thread anyway.
    AAUDIO_CALLBACK_RESULT_CONTINUE
}

/// Called by AAudio (on some thread of its own) when the stream got disconnected
/// (for example, headphones were unplugged) or broke otherwise.
///
/// AAudio's documentation says the stream must not be closed or reopened in here,
/// so this only records the error, and `restart_reason` (polled every frame on the
/// main thread) picks it up.
unsafe extern "C" fn error_callback(
    stream: *mut AAudioStream,
    _user_data: *mut c_void,
    error: i32,
) {
    FAILED_STREAM_ERROR.store(error, Ordering::Relaxed);
    FAILED_STREAM.store(stream, Ordering::Release);
}

/// An open AAudio output stream, playing the audio of an `AudioMixer`.
pub struct AAudioOutput {
    aaudio: &'static AAudio,
    stream: NonNull<AAudioStream>,
    /// Allocated with `Box`, and freed in `drop` after the stream is closed.
    callback_data: NonNull<CallbackData>,
}

impl AAudioOutput {
    /// Opens a 2-channel float AAudio stream at `sample_rate` Hz. It starts paused.
    pub fn open(
        proxy: AudioMixerProxy,
        sample_rate: u32,
        sdk_version: i32,
    ) -> Result<Self, String> {
        let aaudio = AAudio::get(sdk_version)?;
        let callback_data = NonNull::from(Box::leak(Box::new(CallbackData {
            proxy,
            panicked: AtomicBool::new(false),
        })));

        // SAFETY: The callback data stays valid until the stream is closed.
        let stream = match unsafe { open_stream(aaudio, sample_rate, callback_data) } {
            Ok(stream) => stream,
            Err(e) => {
                // SAFETY: No stream was opened, so nothing else can be using it.
                drop(unsafe { Box::from_raw(callback_data.as_ptr()) });
                return Err(e);
            }
        };
        // From here on, `drop` cleans up if something goes wrong.
        let output = Self {
            aaudio,
            stream,
            callback_data,
        };

        // AAudio converts between the requested parameters and the device's ones as
        // needed (the opened stream should use exactly the requested values, or the
        // opening should fail), but make sure, because the mixer depends on them.
        let stream = stream.as_ptr();
        // SAFETY: The stream is open.
        let (actual_rate, channels, format) = unsafe {
            (
                (aaudio.stream_get_sample_rate)(stream),
                (aaudio.stream_get_channel_count)(stream),
                (aaudio.stream_get_format)(stream),
            )
        };
        if actual_rate != sample_rate as i32 {
            return Err(format!(
                "AAudio opened the stream at {actual_rate} Hz instead of {sample_rate} Hz"
            ));
        }
        if channels != CHANNEL_COUNT {
            return Err(format!(
                "AAudio opened the stream with {channels} channels instead of {CHANNEL_COUNT}"
            ));
        }
        if format != AAUDIO_FORMAT_PCM_FLOAT {
            return Err(format!(
                "AAudio opened the stream with format {format} instead of float"
            ));
        }

        Ok(output)
    }

    fn state(&self) -> i32 {
        // SAFETY: The stream is open.
        unsafe { (self.aaudio.stream_get_state)(self.stream.as_ptr()) }
    }

    pub fn start(&mut self) -> Result<(), String> {
        // Like Oboe: requesting the state the stream is already in can fail on some
        // versions ("Avoid state machine errors in O_MR1").
        let state = self.state();
        if state == AAUDIO_STREAM_STATE_STARTING || state == AAUDIO_STREAM_STATE_STARTED {
            return Ok(());
        }
        // SAFETY: The stream is open.
        let result = unsafe { (self.aaudio.stream_request_start)(self.stream.as_ptr()) };
        self.aaudio.check(result, "AAudioStream_requestStart")
    }

    pub fn pause(&mut self) -> Result<(), String> {
        let state = self.state();
        if state == AAUDIO_STREAM_STATE_PAUSING || state == AAUDIO_STREAM_STATE_PAUSED {
            return Ok(());
        }
        // SAFETY: The stream is open.
        let result = unsafe { (self.aaudio.stream_request_pause)(self.stream.as_ptr()) };
        self.aaudio.check(result, "AAudioStream_requestPause")
    }

    /// Returns why the stream has to be reopened, if it does (for example, because the
    /// audio device it was playing on was disconnected). Cheap if nothing is wrong.
    pub fn restart_reason(&self) -> Option<String> {
        if FAILED_STREAM.load(Ordering::Acquire) == self.stream.as_ptr() {
            let error = FAILED_STREAM_ERROR.load(Ordering::Relaxed);
            return Some(format!(
                "AAudio reported an error: {}",
                self.aaudio.result_text(error)
            ));
        }
        if self.state() == AAUDIO_STREAM_STATE_DISCONNECTED {
            return Some("the AAudio stream was disconnected".to_string());
        }
        None
    }
}

/// Opens the stream.
///
/// # Safety
///
/// `callback_data` must stay valid until the returned stream is closed.
unsafe fn open_stream(
    aaudio: &AAudio,
    sample_rate: u32,
    callback_data: NonNull<CallbackData>,
) -> Result<NonNull<AAudioStream>, String> {
    let mut builder = ptr::null_mut();
    aaudio.check(
        (aaudio.create_stream_builder)(&mut builder),
        "AAudio_createStreamBuilder",
    )?;
    if builder.is_null() {
        return Err("AAudio_createStreamBuilder returned null".to_string());
    }

    (aaudio.builder_set_direction)(builder, AAUDIO_DIRECTION_OUTPUT);
    (aaudio.builder_set_format)(builder, AAUDIO_FORMAT_PCM_FLOAT);
    (aaudio.builder_set_channel_count)(builder, CHANNEL_COUNT);
    (aaudio.builder_set_sample_rate)(builder, sample_rate as i32);
    (aaudio.builder_set_performance_mode)(builder, AAUDIO_PERFORMANCE_MODE_LOW_LATENCY);
    if let Some(set_usage) = aaudio.builder_set_usage {
        set_usage(builder, AAUDIO_USAGE_GAME);
    }
    (aaudio.builder_set_data_callback)(
        builder,
        Some(data_callback),
        callback_data.as_ptr() as *mut c_void,
    );
    (aaudio.builder_set_error_callback)(builder, Some(error_callback), ptr::null_mut());

    let mut stream = ptr::null_mut();
    let result = (aaudio.builder_open_stream)(builder, &mut stream);
    (aaudio.builder_delete)(builder);
    aaudio.check(result, "AAudioStreamBuilder_openStream")?;
    NonNull::new(stream).ok_or_else(|| "AAudioStreamBuilder_openStream returned null".to_string())
}

impl Drop for AAudioOutput {
    fn drop(&mut self) {
        let stream = self.stream.as_ptr();
        // SAFETY: The stream is open, and it's not used after being closed here.
        // The callback data is freed only after that, when no callback can run anymore.
        unsafe {
            // Like Oboe: stop, then wait a bit before closing, because closing right
            // away can race with a callback that is still running on some versions.
            // (The result is irrelevant, it fails for example if the stream is disconnected.)
            let _ = (self.aaudio.stream_request_stop)(stream);
            let burst = self
                .aaudio
                .stream_get_frames_per_burst
                .map_or(0, |get_frames_per_burst| get_frames_per_burst(stream));
            let sample_rate = (self.aaudio.stream_get_sample_rate)(stream).max(1);
            let delay_ms =
                (1 + i64::from(burst.max(0)) * 1000 / i64::from(sample_rate)).clamp(10, 100);
            std::thread::sleep(Duration::from_millis(delay_ms as u64));

            let result = (self.aaudio.stream_close)(stream);
            if let Err(e) = self.aaudio.check(result, "AAudioStream_close") {
                log::warn!("{e}");
            }

            drop(Box::from_raw(self.callback_data.as_ptr()));
        }
        let _ = FAILED_STREAM.compare_exchange(
            stream,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
    }
}
