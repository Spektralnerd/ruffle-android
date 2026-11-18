//! Audio output through OpenSL ES (available on every supported Android version), with
//! `libOpenSLES.so` loaded at runtime.
//!
//! The declarations are from the NDK's `SLES/OpenSLES.h` (= Khronos OpenSL ES 1.0.1) and
//! `SLES/OpenSLES_Android.h`, `SLES/OpenSLES_AndroidConfiguration.h`. Only the vtable
//! entries up to the last one used are declared (the rest of each vtable is never read).
//! The usage follows Oboe (`src/opensles/*`), and AOSP's implementation (frameworks/wilhelm).
//!
//! Float samples are only supported by OpenSL ES on Android 5.0+, so this uses 16-bit
//! integer samples, which work everywhere.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError, TryLockError};
use std::time::Duration;

use ruffle_core::backend::audio::AudioMixerProxy;

use super::dl;

/// `SLresult` (`SLuint32`). `SLuint32` is `unsigned int` on Android, on every ABI.
type SlResult = u32;
/// `SLboolean` (`SLuint32`)
type SlBoolean = u32;
/// `SLInterfaceID`: a pointer to a 16-byte struct, which is only passed through here.
type SlInterfaceId = *const c_void;

/// `SLObjectItf` (`const struct SLObjectItf_ * const *`)
type SlObjectItf = *const *const SlObjectVtable;
/// `SLEngineItf`
type SlEngineItf = *const *const SlEngineVtable;
/// `SLPlayItf`
type SlPlayItf = *const *const SlPlayVtable;
/// `SLAndroidSimpleBufferQueueItf`
type SlBufferQueueItf = *const *const SlBufferQueueVtable;
/// `SLAndroidConfigurationItf`
type SlConfigurationItf = *const *const SlConfigurationVtable;

/// A vtable entry that isn't used here.
type UnusedSlot = *const c_void;

const SL_RESULT_SUCCESS: SlResult = 0;
const SL_BOOLEAN_FALSE: SlBoolean = 0;
const SL_BOOLEAN_TRUE: SlBoolean = 1;
const SL_DATALOCATOR_OUTPUTMIX: u32 = 0x0000_0004;
/// From `OpenSLES_Android.h`.
const SL_DATALOCATOR_ANDROIDSIMPLEBUFFERQUEUE: u32 = 0x8000_07BD;
const SL_DATAFORMAT_PCM: u32 = 0x0000_0002;
/// `SL_PCMSAMPLEFORMAT_FIXED_16` (declared as `SLuint16` in the header)
const SL_PCMSAMPLEFORMAT_FIXED_16: u32 = 0x0010;
const SL_SPEAKER_FRONT_LEFT: u32 = 0x0000_0001;
const SL_SPEAKER_FRONT_RIGHT: u32 = 0x0000_0002;
const SL_BYTEORDER_LITTLEENDIAN: u32 = 0x0000_0002;
const SL_PLAYSTATE_PAUSED: u32 = 0x0000_0002;
const SL_PLAYSTATE_PLAYING: u32 = 0x0000_0003;
/// `SL_ANDROID_KEY_STREAM_TYPE`, from `OpenSLES_AndroidConfiguration.h`.
const SL_ANDROID_KEY_STREAM_TYPE: &[u8] = b"androidPlaybackStreamType\0";
/// `SL_ANDROID_STREAM_MEDIA` (`SLint32`), from `OpenSLES_AndroidConfiguration.h`.
const SL_ANDROID_STREAM_MEDIA: i32 = 0x0000_0003;

/// `struct SLObjectItf_` (10 entries)
#[repr(C)]
struct SlObjectVtable {
    /// 0: `SLresult (*Realize)(SLObjectItf self, SLboolean async)`
    realize: unsafe extern "C" fn(SlObjectItf, SlBoolean) -> SlResult,
    /// 1: `Resume`
    _resume: UnusedSlot,
    /// 2: `GetState`
    _get_state: UnusedSlot,
    /// 3: `SLresult (*GetInterface)(SLObjectItf self, const SLInterfaceID iid, void *pInterface)`
    get_interface: unsafe extern "C" fn(SlObjectItf, SlInterfaceId, *mut c_void) -> SlResult,
    /// 4: `RegisterCallback`
    _register_callback: UnusedSlot,
    /// 5: `AbortAsyncOperation`
    _abort_async_operation: UnusedSlot,
    /// 6: `void (*Destroy)(SLObjectItf self)`
    destroy: unsafe extern "C" fn(SlObjectItf),
    // 7: SetPriority, 8: GetPriority, 9: SetLossOfControlInterfaces
}

/// `struct SLEngineItf_` (15 entries)
#[repr(C)]
struct SlEngineVtable {
    /// 0: `CreateLEDDevice`
    _create_led_device: UnusedSlot,
    /// 1: `CreateVibraDevice`
    _create_vibra_device: UnusedSlot,
    /// 2: `SLresult (*CreateAudioPlayer)(SLEngineItf self, SLObjectItf *pPlayer,
    /// SLDataSource *pAudioSrc, SLDataSink *pAudioSnk, SLuint32 numInterfaces,
    /// const SLInterfaceID *pInterfaceIds, const SLboolean *pInterfaceRequired)`
    create_audio_player: unsafe extern "C" fn(
        SlEngineItf,
        *mut SlObjectItf,
        *mut SlDataSource,
        *mut SlDataSink,
        u32,
        *const SlInterfaceId,
        *const SlBoolean,
    ) -> SlResult,
    /// 3: `CreateAudioRecorder`
    _create_audio_recorder: UnusedSlot,
    /// 4: `CreateMidiPlayer`
    _create_midi_player: UnusedSlot,
    /// 5: `CreateListener`
    _create_listener: UnusedSlot,
    /// 6: `Create3DGroup`
    _create_3d_group: UnusedSlot,
    /// 7: `SLresult (*CreateOutputMix)(SLEngineItf self, SLObjectItf *pMix,
    /// SLuint32 numInterfaces, const SLInterfaceID *pInterfaceIds,
    /// const SLboolean *pInterfaceRequired)`
    create_output_mix: unsafe extern "C" fn(
        SlEngineItf,
        *mut SlObjectItf,
        u32,
        *const SlInterfaceId,
        *const SlBoolean,
    ) -> SlResult,
    // 8: CreateMetadataExtractor, 9: CreateExtensionObject, 10: QueryNumSupportedInterfaces,
    // 11: QuerySupportedInterfaces, 12: QueryNumSupportedExtensions,
    // 13: QuerySupportedExtension, 14: IsExtensionSupported
}

/// `struct SLPlayItf_` (12 entries)
#[repr(C)]
struct SlPlayVtable {
    /// 0: `SLresult (*SetPlayState)(SLPlayItf self, SLuint32 state)`
    set_play_state: unsafe extern "C" fn(SlPlayItf, u32) -> SlResult,
    // 1: GetPlayState, 2: GetDuration, 3: GetPosition, 4: RegisterCallback,
    // 5: SetCallbackEventsMask, 6: GetCallbackEventsMask, 7: SetMarkerPosition,
    // 8: ClearMarkerPosition, 9: GetMarkerPosition, 10: SetPositionUpdatePeriod,
    // 11: GetPositionUpdatePeriod
}

/// `slAndroidSimpleBufferQueueCallback`:
/// `void (*)(SLAndroidSimpleBufferQueueItf caller, void *pContext)`
type BufferQueueCallback = unsafe extern "C" fn(SlBufferQueueItf, *mut c_void);

/// `struct SLAndroidSimpleBufferQueueItf_` (4 entries)
#[repr(C)]
struct SlBufferQueueVtable {
    /// 0: `SLresult (*Enqueue)(SLAndroidSimpleBufferQueueItf self, const void *pBuffer,
    /// SLuint32 size)`
    enqueue: unsafe extern "C" fn(SlBufferQueueItf, *const c_void, u32) -> SlResult,
    /// 1: `Clear`
    _clear: UnusedSlot,
    /// 2: `SLresult (*GetState)(SLAndroidSimpleBufferQueueItf self,
    /// SLAndroidSimpleBufferQueueState *pState)`
    get_state: unsafe extern "C" fn(SlBufferQueueItf, *mut SlBufferQueueState) -> SlResult,
    /// 3: `SLresult (*RegisterCallback)(SLAndroidSimpleBufferQueueItf self,
    /// slAndroidSimpleBufferQueueCallback callback, void *pContext)`
    register_callback: unsafe extern "C" fn(
        SlBufferQueueItf,
        Option<BufferQueueCallback>,
        *mut c_void,
    ) -> SlResult,
}

/// `struct SLAndroidConfigurationItf_` (2 entries before API 24, 4 since)
#[repr(C)]
struct SlConfigurationVtable {
    /// 0: `SLresult (*SetConfiguration)(SLAndroidConfigurationItf self,
    /// const SLchar *configKey, const void *pConfigValue, SLuint32 valueSize)`
    set_configuration:
        unsafe extern "C" fn(SlConfigurationItf, *const u8, *const c_void, u32) -> SlResult,
    // 1: GetConfiguration, 2: AcquireJavaProxy (API 24+), 3: ReleaseJavaProxy (API 24+)
}

/// `SLAndroidSimpleBufferQueueState`
#[repr(C)]
#[derive(Default)]
struct SlBufferQueueState {
    count: u32,
    index: u32,
}

/// `SLDataLocator_AndroidSimpleBufferQueue`
#[repr(C)]
struct SlDataLocatorBufferQueue {
    locator_type: u32,
    num_buffers: u32,
}

/// `SLDataLocator_OutputMix`
#[repr(C)]
struct SlDataLocatorOutputMix {
    locator_type: u32,
    output_mix: SlObjectItf,
}

/// `SLDataFormat_PCM`
#[repr(C)]
struct SlDataFormatPcm {
    format_type: u32,
    num_channels: u32,
    /// In milliHertz, despite the name.
    samples_per_sec: u32,
    bits_per_sample: u32,
    container_size: u32,
    channel_mask: u32,
    endianness: u32,
}

/// `SLDataSource`
#[repr(C)]
struct SlDataSource {
    locator: *mut c_void,
    format: *mut c_void,
}

/// `SLDataSink`
#[repr(C)]
struct SlDataSink {
    locator: *mut c_void,
    format: *mut c_void,
}

/// `SLresult slCreateEngine(SLObjectItf *pEngine, SLuint32 numOptions,
/// const SLEngineOption *pEngineOptions, SLuint32 numInterfaces,
/// const SLInterfaceID *pInterfaceIds, const SLboolean *pInterfaceRequired)`
type CreateEngineFn = unsafe extern "C" fn(
    *mut SlObjectItf,
    u32,
    *const c_void,
    u32,
    *const SlInterfaceId,
    *const SlBoolean,
) -> SlResult;

// The layouts above, checked against the C headers (on 32-bit and 64-bit ABIs, all
// pointers, including function pointers, have the same size).
const PTR: usize = size_of::<usize>();
const _: () = {
    use std::mem::offset_of;
    assert!(offset_of!(SlObjectVtable, realize) == 0);
    assert!(offset_of!(SlObjectVtable, get_interface) == 3 * PTR);
    assert!(offset_of!(SlObjectVtable, destroy) == 6 * PTR);
    assert!(offset_of!(SlEngineVtable, create_audio_player) == 2 * PTR);
    assert!(offset_of!(SlEngineVtable, create_output_mix) == 7 * PTR);
    assert!(offset_of!(SlPlayVtable, set_play_state) == 0);
    assert!(offset_of!(SlBufferQueueVtable, enqueue) == 0);
    assert!(offset_of!(SlBufferQueueVtable, get_state) == 2 * PTR);
    assert!(offset_of!(SlBufferQueueVtable, register_callback) == 3 * PTR);
    assert!(offset_of!(SlConfigurationVtable, set_configuration) == 0);
    assert!(size_of::<SlBufferQueueState>() == 8);
    assert!(size_of::<SlDataLocatorBufferQueue>() == 8);
    assert!(size_of::<SlDataLocatorOutputMix>() == 2 * PTR);
    assert!(offset_of!(SlDataLocatorOutputMix, output_mix) == PTR);
    assert!(size_of::<SlDataFormatPcm>() == 7 * 4);
    assert!(size_of::<SlDataSource>() == 2 * PTR);
    assert!(size_of::<SlDataSink>() == 2 * PTR);
};

/// Number of buffers in the buffer queue.
const BUFFER_COUNT: usize = 3;
/// Frames per buffer (about 23 ms at 44.1 kHz).
const BUFFER_FRAMES: usize = 1024;
const CHANNEL_COUNT: usize = 2;
const BUFFER_SAMPLES: usize = BUFFER_FRAMES * CHANNEL_COUNT;
const BUFFER_BYTES: u32 = (BUFFER_SAMPLES * size_of::<i16>()) as u32;

fn check(result: SlResult, what: &str) -> Result<(), String> {
    if result == SL_RESULT_SUCCESS {
        Ok(())
    } else {
        Err(format!("{what} failed: OpenSL ES error {result:#x}"))
    }
}

/// The OpenSL ES entry point and interface IDs, loaded from `libOpenSLES.so`.
struct OpenSles {
    create_engine: CreateEngineFn,
    iid_engine: SlInterfaceId,
    iid_play: SlInterfaceId,
    iid_buffer_queue: SlInterfaceId,
    iid_android_simple_buffer_queue: SlInterfaceId,
    iid_android_configuration: Option<SlInterfaceId>,
}

// SAFETY: The interface IDs point to immutable data in libOpenSLES.so, which is never unloaded.
unsafe impl Send for OpenSles {}
unsafe impl Sync for OpenSles {}

impl OpenSles {
    /// Loads `libOpenSLES.so` once per process.
    fn get() -> Result<&'static Self, String> {
        static OPENSLES: OnceLock<Result<OpenSles, String>> = OnceLock::new();
        OPENSLES
            .get_or_init(|| {
                // SAFETY: The types match the declarations in the OpenSL ES headers.
                unsafe { Self::load() }
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    unsafe fn load() -> Result<Self, String> {
        let lib = dl::Library::open("libOpenSLES.so\0")?;
        // Each `SL_IID_X` symbol is a `const SLInterfaceID` variable, so `dlsym` returns
        // the address of the pointer, which has to be read (like in Oboe).
        let iid = |name: &str| -> Result<SlInterfaceId, String> {
            let address = lib.symbol(name)?.as_ptr() as *const SlInterfaceId;
            let iid = *address;
            if iid.is_null() {
                Err(format!("{} is null", name.trim_end_matches('\0')))
            } else {
                Ok(iid)
            }
        };
        Ok(Self {
            create_engine: lib.function("slCreateEngine\0")?,
            iid_engine: iid("SL_IID_ENGINE\0")?,
            iid_play: iid("SL_IID_PLAY\0")?,
            iid_buffer_queue: iid("SL_IID_BUFFERQUEUE\0")?,
            iid_android_simple_buffer_queue: iid("SL_IID_ANDROIDSIMPLEBUFFERQUEUE\0")?,
            iid_android_configuration: iid("SL_IID_ANDROIDCONFIGURATION\0").ok(),
        })
    }
}

/// The OpenSL ES engine object. Android allows only one per process, so it's shared,
/// and destroyed when the last user is gone (like in Oboe).
struct Engine {
    object: SlObjectItf,
    itf: SlEngineItf,
    users: usize,
}

// SAFETY: The engine is thread-safe (that's the default), and only accessed under a lock here.
unsafe impl Send for Engine {}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

/// A reference to the shared engine.
struct EngineRef {
    itf: SlEngineItf,
}

impl EngineRef {
    fn acquire(sles: &OpenSles) -> Result<Self, String> {
        let mut engine = ENGINE.lock().unwrap_or_else(PoisonError::into_inner);
        let mut state = match engine.take() {
            Some(state) => state,
            // SAFETY: The function and IID were loaded from libOpenSLES.so.
            None => unsafe { create_engine(sles)? },
        };
        state.users += 1;
        let itf = state.itf;
        *engine = Some(state);
        Ok(Self { itf })
    }
}

impl Drop for EngineRef {
    fn drop(&mut self) {
        let mut engine = ENGINE.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(state) = engine.as_mut() {
            state.users -= 1;
            if state.users == 0 {
                let object = state.object;
                *engine = None;
                // SAFETY: Nothing uses the engine anymore.
                unsafe { ((**object).destroy)(object) };
            }
        }
    }
}

unsafe fn create_engine(sles: &OpenSles) -> Result<Engine, String> {
    let mut object: SlObjectItf = ptr::null();
    check(
        (sles.create_engine)(&mut object, 0, ptr::null(), 0, ptr::null(), ptr::null()),
        "slCreateEngine",
    )?;
    if object.is_null() {
        return Err("slCreateEngine returned null".to_string());
    }
    let mut itf: SlEngineItf = ptr::null();
    let result = check(
        ((**object).realize)(object, SL_BOOLEAN_FALSE),
        "Realize (engine)",
    )
    .and_then(|()| {
        check(
            ((**object).get_interface)(
                object,
                sles.iid_engine,
                &mut itf as *mut SlEngineItf as *mut c_void,
            ),
            "GetInterface (engine)",
        )
    })
    .and_then(|()| {
        if itf.is_null() {
            Err("GetInterface (engine) returned null".to_string())
        } else {
            Ok(())
        }
    });
    match result {
        Ok(()) => Ok(Engine {
            object,
            itf,
            users: 0,
        }),
        Err(e) => {
            ((**object).destroy)(object);
            Err(e)
        }
    }
}

/// The data shared with the buffer queue callback. It's only ever accessed through
/// shared references, and it's freed only after the player object is destroyed.
struct CallbackData {
    proxy: AudioMixerProxy,
    panicked: AtomicBool,
    /// Set if a buffer couldn't be enqueued, which breaks the callback chain.
    enqueue_failed: AtomicBool,
    buffers: Mutex<Buffers>,
}

struct Buffers {
    /// The buffers of the queue, in separate allocations, so that writing one never
    /// touches memory that OpenSL ES may be reading.
    pcm: [Box<[i16]>; BUFFER_COUNT],
    /// The mixer output, which gets converted to `pcm` (mixing into `i16` directly could overflow).
    mix: Box<[f32]>,
    /// The buffer to fill next. The buffers are enqueued in order, and OpenSL ES plays
    /// them in that order, so the queued ones are always the `count` buffers before this
    /// one (cyclically), and this one is free if fewer than `BUFFER_COUNT` are queued.
    next: usize,
}

impl CallbackData {
    /// Fills and enqueues the next buffer, if the queue isn't full.
    ///
    /// # Safety
    ///
    /// `queue` must be the buffer queue interface of the player this data belongs to.
    unsafe fn enqueue_next(&self, buffers: &mut Buffers, queue: SlBufferQueueItf) -> SlResult {
        let mut state = SlBufferQueueState::default();
        let result = ((**queue).get_state)(queue, &mut state);
        if result != SL_RESULT_SUCCESS {
            return result;
        }
        if state.count as usize >= BUFFER_COUNT {
            return SL_RESULT_SUCCESS;
        }

        super::mix(&self.proxy, &mut buffers.mix, &self.panicked);
        let pcm = &mut buffers.pcm[buffers.next];
        for (out, &sample) in pcm.iter_mut().zip(buffers.mix.iter()) {
            // `as` saturates (and maps NaN to 0).
            *out = (sample * 32767.0) as i16;
        }
        let result = ((**queue).enqueue)(queue, pcm.as_ptr() as *const c_void, BUFFER_BYTES);
        if result == SL_RESULT_SUCCESS {
            buffers.next = (buffers.next + 1) % BUFFER_COUNT;
        }
        result
    }
}

/// Called by OpenSL ES (on an audio thread of the OS) whenever it finished playing a buffer.
unsafe extern "C" fn buffer_queue_callback(queue: SlBufferQueueItf, context: *mut c_void) {
    if context.is_null() {
        return;
    }
    // SAFETY: `context` is the `CallbackData` of this player, which outlives the player.
    let data = &*(context as *const CallbackData);
    // If the lock is held, the main thread is filling the queue right now, so there's
    // nothing to do. (Never block here.)
    let mut buffers = match data.buffers.try_lock() {
        Ok(buffers) => buffers,
        Err(TryLockError::Poisoned(e)) => e.into_inner(),
        Err(TryLockError::WouldBlock) => return,
    };
    if data.enqueue_next(&mut buffers, queue) != SL_RESULT_SUCCESS {
        data.enqueue_failed.store(true, Ordering::Relaxed);
    }
}

/// An OpenSL ES audio player, playing the audio of an `AudioMixer`.
pub struct OpenSlesOutput {
    output_mix: SlObjectItf,
    player: SlObjectItf,
    play: SlPlayItf,
    queue: SlBufferQueueItf,
    /// Allocated with `Box`, and freed in `drop` after the player is destroyed.
    callback_data: NonNull<CallbackData>,
    /// Must be released after the objects above are destroyed (fields are dropped
    /// after `Drop::drop` runs).
    engine: EngineRef,
}

impl OpenSlesOutput {
    /// Creates a 2-channel 16-bit player at `sample_rate` Hz. It starts paused.
    pub fn open(proxy: AudioMixerProxy, sample_rate: u32) -> Result<Self, String> {
        let sles = OpenSles::get()?;
        let engine = EngineRef::acquire(sles)?;
        let callback_data = NonNull::from(Box::leak(Box::new(CallbackData {
            proxy,
            panicked: AtomicBool::new(false),
            enqueue_failed: AtomicBool::new(false),
            buffers: Mutex::new(Buffers {
                pcm: [
                    vec![0; BUFFER_SAMPLES].into_boxed_slice(),
                    vec![0; BUFFER_SAMPLES].into_boxed_slice(),
                    vec![0; BUFFER_SAMPLES].into_boxed_slice(),
                ],
                mix: vec![0.0; BUFFER_SAMPLES].into_boxed_slice(),
                next: 0,
            }),
        })));
        let mut output = Self {
            output_mix: ptr::null(),
            player: ptr::null(),
            play: ptr::null(),
            queue: ptr::null(),
            callback_data,
            engine,
        };
        // On failure, `drop` destroys whatever was created.
        // SAFETY: `output` holds a reference to the engine, and the loaded functions.
        unsafe { output.init(sles, sample_rate)? };
        Ok(output)
    }

    unsafe fn init(&mut self, sles: &OpenSles, sample_rate: u32) -> Result<(), String> {
        let engine = self.engine.itf;

        check(
            ((**engine).create_output_mix)(
                engine,
                &mut self.output_mix,
                0,
                ptr::null(),
                ptr::null(),
            ),
            "CreateOutputMix",
        )?;
        if self.output_mix.is_null() {
            return Err("CreateOutputMix returned null".to_string());
        }
        check(
            ((**self.output_mix).realize)(self.output_mix, SL_BOOLEAN_FALSE),
            "Realize (output mix)",
        )?;

        let mut locator = SlDataLocatorBufferQueue {
            locator_type: SL_DATALOCATOR_ANDROIDSIMPLEBUFFERQUEUE,
            num_buffers: BUFFER_COUNT as u32,
        };
        let mut format = SlDataFormatPcm {
            format_type: SL_DATAFORMAT_PCM,
            num_channels: CHANNEL_COUNT as u32,
            samples_per_sec: sample_rate * 1000,
            bits_per_sample: SL_PCMSAMPLEFORMAT_FIXED_16,
            container_size: SL_PCMSAMPLEFORMAT_FIXED_16,
            channel_mask: SL_SPEAKER_FRONT_LEFT | SL_SPEAKER_FRONT_RIGHT,
            endianness: SL_BYTEORDER_LITTLEENDIAN,
        };
        let mut source = SlDataSource {
            locator: &mut locator as *mut SlDataLocatorBufferQueue as *mut c_void,
            format: &mut format as *mut SlDataFormatPcm as *mut c_void,
        };
        let mut output_mix_locator = SlDataLocatorOutputMix {
            locator_type: SL_DATALOCATOR_OUTPUTMIX,
            output_mix: self.output_mix,
        };
        let mut sink = SlDataSink {
            locator: &mut output_mix_locator as *mut SlDataLocatorOutputMix as *mut c_void,
            format: ptr::null_mut(),
        };
        // Like Oboe: request the standard buffer queue interface (on an audio player,
        // the Android simple buffer queue is an alias for it), and the configuration.
        let mut interface_ids = [sles.iid_buffer_queue, ptr::null()];
        let required = [SL_BOOLEAN_TRUE, SL_BOOLEAN_FALSE];
        let mut interface_count = 1;
        if let Some(iid) = sles.iid_android_configuration {
            interface_ids[1] = iid;
            interface_count = 2;
        }
        check(
            ((**engine).create_audio_player)(
                engine,
                &mut self.player,
                &mut source,
                &mut sink,
                interface_count,
                interface_ids.as_ptr(),
                required.as_ptr(),
            ),
            "CreateAudioPlayer",
        )?;
        if self.player.is_null() {
            return Err("CreateAudioPlayer returned null".to_string());
        }
        let player = self.player;

        // Play on the media stream (this has to be set before realizing the player).
        // That's already the default, so it's fine if this fails.
        if let Some(iid) = sles.iid_android_configuration {
            let mut config: SlConfigurationItf = ptr::null();
            let result = ((**player).get_interface)(
                player,
                iid,
                &mut config as *mut SlConfigurationItf as *mut c_void,
            );
            if result == SL_RESULT_SUCCESS && !config.is_null() {
                let stream_type = SL_ANDROID_STREAM_MEDIA;
                let result = ((**config).set_configuration)(
                    config,
                    SL_ANDROID_KEY_STREAM_TYPE.as_ptr(),
                    &stream_type as *const i32 as *const c_void,
                    size_of::<i32>() as u32,
                );
                if let Err(e) = check(result, "SetConfiguration (stream type)") {
                    log::warn!("{e}");
                }
            }
        }

        check(
            ((**player).realize)(player, SL_BOOLEAN_FALSE),
            "Realize (audio player)",
        )?;
        check(
            ((**player).get_interface)(
                player,
                sles.iid_play,
                &mut self.play as *mut SlPlayItf as *mut c_void,
            ),
            "GetInterface (play)",
        )?;
        check(
            ((**player).get_interface)(
                player,
                sles.iid_android_simple_buffer_queue,
                &mut self.queue as *mut SlBufferQueueItf as *mut c_void,
            ),
            "GetInterface (buffer queue)",
        )?;
        if self.play.is_null() || self.queue.is_null() {
            return Err("GetInterface returned null".to_string());
        }
        check(
            ((**self.queue).register_callback)(
                self.queue,
                Some(buffer_queue_callback),
                self.callback_data.as_ptr() as *mut c_void,
            ),
            "RegisterCallback (buffer queue)",
        )
    }

    fn callback_data(&self) -> &CallbackData {
        // SAFETY: Valid until `drop`.
        unsafe { self.callback_data.as_ref() }
    }

    /// Fills the buffer queue (it's empty initially, and it can run empty if the callback
    /// chain broke). Without queued buffers, the buffer queue callback is never called.
    fn fill_queue(&self) -> Result<(), String> {
        let data = self.callback_data();
        let mut buffers = data.buffers.lock().unwrap_or_else(PoisonError::into_inner);
        for _ in 0..BUFFER_COUNT {
            // SAFETY: This is the buffer queue of the player.
            let result = unsafe { data.enqueue_next(&mut buffers, self.queue) };
            if let Err(e) = check(result, "Enqueue") {
                data.enqueue_failed.store(true, Ordering::Relaxed);
                return Err(e);
            }
        }
        Ok(())
    }

    fn set_play_state(&self, state: u32, what: &str) -> Result<(), String> {
        // SAFETY: The play interface is valid until `drop`.
        check(
            unsafe { ((**self.play).set_play_state)(self.play, state) },
            what,
        )
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.fill_queue()?;
        self.set_play_state(SL_PLAYSTATE_PLAYING, "SetPlayState (playing)")
    }

    pub fn pause(&mut self) -> Result<(), String> {
        // The queued buffers are kept, and played when resuming.
        self.set_play_state(SL_PLAYSTATE_PAUSED, "SetPlayState (paused)")
    }

    /// Returns why the player has to be recreated, if it does. Cheap if nothing is wrong.
    ///
    /// Unlike AAudio streams, OpenSL ES players follow audio device changes by themselves,
    /// so the only reason is a broken callback chain (which should never happen).
    pub fn restart_reason(&self) -> Option<String> {
        if self.callback_data().enqueue_failed.load(Ordering::Relaxed) {
            Some("an OpenSL ES buffer couldn't be enqueued".to_string())
        } else {
            None
        }
    }
}

impl Drop for OpenSlesOutput {
    fn drop(&mut self) {
        // SAFETY: Each object is destroyed once, and not used afterwards. Destroying the
        // player waits for a running callback to finish, and prevents further ones, so
        // the callback data can be freed after that.
        unsafe {
            if !self.player.is_null() {
                if !self.play.is_null() {
                    // Like Oboe: pause, and give a running callback some time to finish.
                    let _ = ((**self.play).set_play_state)(self.play, SL_PLAYSTATE_PAUSED);
                    std::thread::sleep(Duration::from_millis(25));
                }
                ((**self.player).destroy)(self.player);
            }
            if !self.output_mix.is_null() {
                ((**self.output_mix).destroy)(self.output_mix);
            }
            drop(Box::from_raw(self.callback_data.as_ptr()));
        }
    }
}
