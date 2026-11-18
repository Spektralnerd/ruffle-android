mod audio;
mod custom_event;
mod input;
mod java;
mod keycodes;
mod navigator;
mod storage;
mod trace;
mod ui;

use custom_event::RuffleEvent;

use jni::{
    errors::ThrowRuntimeExAndDefault,
    jni_sig, jni_str,
    objects::{JObject, JString},
    sys::{self, jint, jobject},
    Env, EnvUnowned, JavaVM,
};
use keycodes::{
    android_key_to_gamepad_button, android_key_to_ruffle_key_descriptor,
    android_key_to_text_control, char_to_key_descriptor, default_gamepad_button_mapping,
    key_tag_to_key_descriptor, named_key_descriptor,
};
use std::any::Any;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::MutexGuard;
use std::time::Duration;
use std::{
    panic,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};
use wgpu::rwh::{AndroidDisplayHandle, HasWindowHandle, RawDisplayHandle};

use android_activity::input::{
    Axis, Button, ImeOptions, InputEvent, InputType, KeyAction, KeyEvent, KeyMapChar, MotionAction,
    TextInputAction, TextInputState, TextSpan,
};
use android_activity::{AndroidApp, AndroidAppWaker, InputStatus, MainEvent, PollEvent};
use backtrace::Backtrace;
use jni::objects::JClass;
use ndk::native_window::NativeWindow;

use audio::AndroidAudioBackend;
use url::Url;

use ruffle_common::duration::FloatDuration;
use ruffle_core::{
    backend::audio::NullAudioBackend,
    backend::navigator::OwnedFuture,
    events::{
        LogicalKey, MouseButton, MouseWheelDelta, NamedKey, PhysicalKey, PlayerEvent,
        TextControlCode,
    },
    tag_utils::SwfMovieData,
    Player, PlayerBuilder, ViewportDimensions,
};
use ruffle_frontend_utils::content::PlayingContent;
use ruffle_frontend_utils::{
    backends::navigator::{ExternalNavigatorBackend, FutureSpawner},
    content::ContentDescriptor,
};

use crate::input::ImeTextTracker;
use crate::navigator::AndroidNavigatorInterface;
use crate::storage::{shared_object_prefix, AndroidStorageBackend};
use crate::trace::FileLogBackend;
use crate::ui::{AndroidUiBackend, ErrorKind};
use java::JavaInterface;
use ruffle_render_wgpu::{backend::WgpuRenderBackend, target::SwapChainTarget};

/// A unique identifier for a given `Player` instance.
/// Used to track which player any currently executing future is bound to.
#[derive(Copy, Clone, Eq, PartialEq)]
struct PlayerId(i64);

impl PlayerId {
    fn new() -> Self {
        use std::sync::atomic::{AtomicI64, Ordering};

        static NEXT: AtomicI64 = AtomicI64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        assert!(id >= 0, "PlayerId overflowed!");
        Self(id)
    }
}

/// A `Player`-bound future that is currently running.
pub struct PlayerRunnable(async_task::Runnable<PlayerId>);

/// Represents a current Player and any associated state with that player,
/// which may be lost when this Player is closed (dropped)
struct ActivePlayer {
    id: PlayerId,
    player: Arc<Mutex<Player>>,
}

#[derive(Clone)]
pub struct EventSender {
    sender: Sender<RuffleEvent>,
    waker: AndroidAppWaker,
}

impl EventSender {
    pub fn send(&self, event: RuffleEvent) {
        if self.sender.send(event).is_ok() {
            self.waker.wake();
        }
    }
}

/// A bare-bones executor that schedules tasks on the winit event loop.
struct AndroidExecutor {
    event_loop: EventSender,
    player_id: PlayerId,
}

impl<E: std::error::Error + 'static> FutureSpawner<E> for AndroidExecutor {
    fn spawn(&self, future: OwnedFuture<(), E>) {
        // Discard any errors.
        let future = async {
            if let Err(e) = future.await {
                tracing::error!("Async error: {}", e);
            }
        };

        let event_loop = self.event_loop.clone();
        let scheduler = move |task| {
            let event = RuffleEvent::TaskPoll(PlayerRunnable(task));
            event_loop.send(event)
        };

        let (runnable, task) = async_task::Builder::new()
            .metadata(self.player_id)
            .spawn_local(|_| future, scheduler);

        // The future should run in the background.
        task.detach();
        // Immediately schedule the future to be polled for the first time.
        runnable.schedule();
    }
}

/// Everything the player needs from the activity, read once at startup.
struct StartupInfo {
    trace_output: Option<PathBuf>,
    storage_dir: PathBuf,
}

/// The URL the movie gets inside Ruffle.
///
/// Files opened through the system file picker have `content://` URLs, which can differ each
/// time the same file is opened. Ruffle derives where a movie's saves (SharedObjects) are
/// stored from its URL, so those would be lost. These movies get a URL based on their file
/// name instead, which stays the same. (It keeps the `content` scheme, so that the movie stays
/// in the same security sandbox as before.)
fn movie_url_for(swf_uri: &str, display_name: Option<&str>) -> String {
    match display_name {
        Some(name) if swf_uri.starts_with("content:") && !name.is_empty() => {
            let mut url = Url::parse("content://rs.ruffle.local/").expect("valid URL");
            if let Ok(mut segments) = url.path_segments_mut() {
                segments.clear().push(name);
            }
            url.to_string()
        }
        _ => swf_uri.to_owned(),
    }
}

fn viewport_dimensions(app: &AndroidApp, window: &NativeWindow) -> ViewportDimensions {
    let scale_factor = app
        .config()
        .density()
        .map(|dpi| dpi as f64 / 160.0)
        .unwrap_or(1.0);
    ViewportDimensions {
        width: window.width().max(1) as u32,
        height: window.height().max(1) as u32,
        scale_factor,
    }
}

fn surface_target(window: &NativeWindow) -> Option<wgpu::SurfaceTargetUnsafe> {
    Some(wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: window.window_handle().ok()?.into(),
    })
}

/// Points the renderer of an existing player to a new (or resized) window surface.
fn recreate_surface(player: &mut Player, window: &NativeWindow) {
    let Some(renderer) =
        <dyn Any>::downcast_mut::<WgpuRenderBackend<SwapChainTarget>>(player.renderer_mut())
    else {
        return;
    };
    let Some(target) = surface_target(window) else {
        log::error!("No window handle to recreate the surface with");
        return;
    };
    let size = (window.width() as u32, window.height() as u32);
    // SAFETY: The window stays valid until we're told it's terminated, and then we either
    // stop rendering, or get a new window and recreate the surface again.
    if let Err(e) = unsafe { renderer.recreate_surface_unsafe(target, size) } {
        log::error!("Couldn't recreate the surface: {e}");
    }
}

fn flush_shared_objects(playerbox: &Option<ActivePlayer>) {
    if let Some(player) = playerbox {
        if let Ok(mut player) = player.player.lock() {
            player.flush_shared_objects();
        }
    }
}

/// Creates the player for the movie the activity was started with, and starts loading it.
fn create_player(
    app: &AndroidApp,
    window: &NativeWindow,
    sender: &EventSender,
    startup: &StartupInfo,
) -> Option<ActivePlayer> {
    let dimensions = viewport_dimensions(app, window);

    let renderer = surface_target(window).ok_or_else(|| "no window handle".to_string());
    let renderer = renderer.and_then(|target| unsafe {
        // SAFETY: See `recreate_surface`.
        WgpuRenderBackend::for_window_unsafe(
            target,
            (dimensions.width, dimensions.height),
            wgpu::Backends::GL,
            wgpu::PowerPreference::HighPerformance,
            None,
        )
        .map_err(|e| e.to_string())
    });
    let renderer = match renderer {
        Ok(renderer) => renderer,
        Err(e) => {
            ui::show_error(app, ErrorKind::RendererFailed, &e);
            return None;
        }
    };

    let (swf_uri, swf_bytes, display_name) = match with_activity(app, |env, activity| {
        (
            JavaInterface::get_swf_uri(env, activity),
            JavaInterface::get_swf_bytes(env, activity),
            JavaInterface::get_swf_display_name(env, activity),
        )
    }) {
        Ok(info) => info,
        Err(e) => {
            ui::show_error(app, ErrorKind::MovieLoadFailed, &e.to_string());
            return None;
        }
    };
    let movie_url = movie_url_for(&swf_uri, display_name.as_deref());
    // Saves made while the movie still had its `content://` URL.
    let legacy_prefix = if movie_url != swf_uri {
        shared_object_prefix(&movie_url).zip(shared_object_prefix(&swf_uri))
    } else {
        None
    };
    log::info!("Starting movie {swf_uri} as {movie_url}");

    let base_url = Url::parse(&movie_url)
        .unwrap_or_else(|_| Url::parse("file://movie.swf").expect("valid URL"));
    let player_id = PlayerId::new();
    let future_spawner = AndroidExecutor {
        event_loop: sender.clone(),
        player_id,
    };
    let navigator = ExternalNavigatorBackend::new(
        base_url.clone(),
        None,
        None,
        future_spawner,
        None,
        true,
        Default::default(),
        ruffle_core::backend::navigator::SocketMode::Allow,
        Rc::new(PlayingContent::DirectFile(ContentDescriptor::new_remote(
            base_url,
        ))),
        AndroidNavigatorInterface,
    );

    let builder = PlayerBuilder::new()
        .with_renderer(renderer)
        .with_ui(AndroidUiBackend::new(app.clone(), sender.clone()))
        .with_storage(Box::new(AndroidStorageBackend::new(
            startup.storage_dir.clone(),
            legacy_prefix,
        )))
        .with_navigator(navigator)
        .with_log(FileLogBackend::new(startup.trace_output.as_deref()))
        .with_video(ruffle_video_software::backend::SoftwareVideoBackend::new())
        .with_gamepad_button_mapping(default_gamepad_button_mapping());
    let builder = match AndroidAudioBackend::new(app.config().sdk_version()) {
        Ok(audio) => builder.with_audio(audio),
        Err(e) => {
            log::error!("No audio output available, playing without sound: {e}");
            builder.with_boxed_audio(Box::new(NullAudioBackend::new()))
        }
    };
    let player = builder.build();

    {
        let mut player_lock = player.lock().unwrap();
        if let Some(bytes) = swf_bytes {
            match SwfMovieData::from_data(&bytes, movie_url, None, None) {
                Ok(movie) => {
                    player_lock.mutate_with_update_context(|context| {
                        context.set_root_movie(movie);
                    });
                    sender.send(RuffleEvent::RootMovieLoaded);
                }
                Err(e) => ui::show_error(app, ErrorKind::InvalidMovie, &e.to_string()),
            }
        } else {
            // Failures are reported by Ruffle through `UiBackend`.
            let sender = sender.clone();
            player_lock.fetch_root_movie(
                swf_uri,
                Vec::new(),
                Box::new(move |_header| sender.send(RuffleEvent::RootMovieLoaded)),
            );
        }
        player_lock.set_is_playing(true); // Desktop player will auto-play.
        player_lock.set_letterbox(ruffle_core::config::Letterbox::On);
        player_lock.set_viewport_dimensions(dimensions);
    }

    log::info!("MOVIE STARTED");
    Some(ActivePlayer {
        id: player_id,
        player,
    })
}

/// The character a key produces on its keyboard, with the current modifiers.
///
/// `Err` if the keyboard's character map isn't available (e.g. for injected key events).
fn key_character(app: &AndroidApp, event: &KeyEvent) -> Result<Option<char>, ()> {
    let map = app
        .device_key_character_map(event.device_id())
        .map_err(|_| ())?;
    match map.get(event.key_code(), event.meta_state()) {
        Ok(KeyMapChar::Unicode(c)) => Ok(Some(c)),
        Ok(_) => Ok(None),
        Err(_) => Err(()),
    }
}

/// Presses and releases a key, e.g. for keys typed on the soft keyboard.
fn tap_key(
    player: &mut Player,
    key: ruffle_core::events::KeyDescriptor,
    text: Option<PlayerEvent>,
) {
    player.handle_event(PlayerEvent::KeyDown { key });
    if let Some(text) = text {
        player.handle_event(text);
    }
    player.handle_event(PlayerEvent::KeyUp { key });
}

fn tap_enter(player: &mut Player) {
    tap_key(
        player,
        named_key_descriptor(PhysicalKey::Enter, NamedKey::Enter),
        Some(PlayerEvent::TextControl {
            code: TextControlCode::Enter,
        }),
    );
}

fn handle_key_event(app: &AndroidApp, player: &ActivePlayer, event: &KeyEvent) -> InputStatus {
    let key_code = event.key_code();
    let down = match event.action() {
        KeyAction::Down => true,
        KeyAction::Up => false,
        _ => return InputStatus::Unhandled,
    };

    if let Some(button) = android_key_to_gamepad_button(key_code) {
        if !down || event.repeat_count() == 0 {
            let event = if down {
                PlayerEvent::GamepadButtonDown { button }
            } else {
                PlayerEvent::GamepadButtonUp { button }
            };
            player.player.lock().unwrap().handle_event(event);
        }
        return InputStatus::Handled;
    }

    let character = key_character(app, event);
    let Some(key) = android_key_to_ruffle_key_descriptor(key_code, character.ok().flatten()) else {
        return InputStatus::Unhandled;
    };

    let mut player = player.player.lock().unwrap();
    if !down {
        player.handle_event(PlayerEvent::KeyUp { key });
        return InputStatus::Handled;
    }

    player.handle_event(PlayerEvent::KeyDown { key });
    let meta = event.meta_state();
    if let Some(code) = android_key_to_text_control(key_code, meta) {
        player.handle_event(PlayerEvent::TextControl { code });
    } else if !meta.ctrl_on() && !meta.meta_on() {
        // Without a character map, fall back to the (unshifted) character of the key.
        let text = match (character, key.logical_key) {
            (Ok(c), _) => c,
            (Err(()), LogicalKey::Character(c)) => Some(c),
            (Err(()), _) => None,
        };
        if let Some(codepoint) = text.filter(|c| !c.is_control()) {
            player.handle_event(PlayerEvent::TextInput { codepoint });
        }
    }
    InputStatus::Handled
}

/// Replays what was typed on the soft keyboard as key presses and text input.
fn handle_soft_keyboard_text(player: &ActivePlayer, ime: &mut ImeTextTracker, text: &str) {
    let edit = ime.update(text);
    if edit.backspaces == 0 && edit.inserted.is_empty() {
        return;
    }
    let mut player = player.player.lock().unwrap();
    for _ in 0..edit.backspaces {
        tap_key(
            &mut player,
            named_key_descriptor(PhysicalKey::Backspace, NamedKey::Backspace),
            Some(PlayerEvent::TextControl {
                code: TextControlCode::Backspace,
            }),
        );
    }
    for c in edit.inserted.chars() {
        if c == '\n' {
            tap_enter(&mut player);
        } else {
            tap_key(
                &mut player,
                char_to_key_descriptor(c),
                Some(PlayerEvent::TextInput { codepoint: c }),
            );
        }
    }
}

fn handle_motion_event(
    app: &AndroidApp,
    player: &ActivePlayer,
    window: &NativeWindow,
    event: &android_activity::input::MotionEvent,
) -> InputStatus {
    let pointer = event.pointer_at_index(event.pointer_index());
    let (loc_x, loc_y) = get_loc_in_window(app);
    let Ok((view_width, view_height)) = get_view_size(app) else {
        return InputStatus::Unhandled;
    };
    if view_width <= 0 || view_height <= 0 {
        return InputStatus::Unhandled;
    }
    let x = (pointer.x() as f64 - loc_x as f64) * window.width() as f64 / view_width as f64;
    let y = (pointer.y() as f64 - loc_y as f64) * window.height() as f64 / view_height as f64;

    let ruffle_event = match event.action() {
        MotionAction::Down | MotionAction::PointerDown => PlayerEvent::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            index: None,
        },
        MotionAction::Up | MotionAction::PointerUp => PlayerEvent::MouseUp {
            x,
            y,
            button: MouseButton::Left,
        },
        // A mouse also sends Down/Up for its primary button, so only the others are handled
        // here.
        MotionAction::ButtonPress | MotionAction::ButtonRelease => {
            let button = match event.action_button() {
                Button::Secondary | Button::StylusSecondary => MouseButton::Right,
                Button::Tertiary => MouseButton::Middle,
                _ => return InputStatus::Handled,
            };
            if event.action() == MotionAction::ButtonPress {
                PlayerEvent::MouseDown {
                    x,
                    y,
                    button,
                    index: None,
                }
            } else {
                PlayerEvent::MouseUp { x, y, button }
            }
        }
        MotionAction::Move | MotionAction::HoverMove => PlayerEvent::MouseMove { x, y },
        MotionAction::HoverExit => PlayerEvent::MouseLeave,
        MotionAction::Scroll => PlayerEvent::MouseWheel {
            delta: MouseWheelDelta::Lines(pointer.axis_value(Axis::Vscroll) as f64),
        },
        _ => return InputStatus::Unhandled,
    };

    player.player.lock().unwrap().handle_event(ruffle_event);
    InputStatus::Handled
}

#[tokio::main]
async fn run(app: AndroidApp) {
    let mut last_frame_time = Instant::now();
    let mut next_frame_time = Some(Instant::now());
    let mut quit = false;
    let (sender, receiver) = mpsc::channel::<RuffleEvent>();
    let mut native_window: Option<NativeWindow> = None;
    let mut playerbox: Option<ActivePlayer> = None;
    let mut ime = ImeTextTracker::default();
    let mut soft_keyboard_open = false;
    let sender = EventSender {
        sender,
        waker: app.create_waker(),
    };

    log::info!("Starting event loop...");
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut sys::JavaVM) };
    // This is a global reference, so it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;

    let startup = vm
        .attach_current_thread(|env| -> jni::errors::Result<_> {
            let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
            let trace_output = JavaInterface::get_trace_output(env, &activity);
            let storage_dir = JavaInterface::get_android_data_storage_dir(env, &activity);
            let _ = unsafe {
                env.set_rust_field(&*activity, jni_str!("eventLoopHandle"), sender.clone())
            };
            // Lets reqwest verify server certificates using Android's trust store.
            let context = env.new_local_ref(&*activity)?;
            rustls_platform_verifier::android::init_with_env(env, context)?;
            Ok(StartupInfo {
                trace_output,
                storage_dir,
            })
        })
        .expect("JNI calls on the main thread must succeed");

    // Mouse wheels (only X and Y are reported by default).
    app.enable_motion_axis(Axis::Vscroll);
    // Soft keyboard: multi-line, so that Enter is typed as "\n" (instead of being sent as an
    // editor action, twice for hardware keys), and never as a fullscreen editor covering the
    // movie in landscape.
    let mut ime_options = ImeOptions::IME_FLAG_NO_FULLSCREEN | ImeOptions::IMG_FLAG_NO_EXTRACT_UI;
    ime_options.set_action(TextInputAction::None);
    app.set_ime_editor_info(
        InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_MULTI_LINE,
        TextInputAction::None,
        ime_options,
    );

    while !quit {
        let mut needs_redraw = false;
        app.poll_events(
            Some(
                next_frame_time
                    .and_then(|next| next.checked_duration_since(last_frame_time))
                    .unwrap_or_else(|| Duration::from_millis(100)),
            ),
            |event| {
                match event {
                    PollEvent::Main(event) => match event {
                        MainEvent::Destroy => {
                            flush_shared_objects(&playerbox);
                            quit = true;
                        }
                        // The app may be killed any time after it goes to the background,
                        // without getting destroyed first, so save while we still can.
                        MainEvent::Pause | MainEvent::SaveState { .. } | MainEvent::Stop => {
                            flush_shared_objects(&playerbox);
                        }
                        MainEvent::WindowResized { .. } => {
                            if let (Some(player), Some(window)) =
                                (playerbox.as_ref(), native_window.as_ref())
                            {
                                log::info!(
                                    "WindowResized: {} x {}",
                                    window.width(),
                                    window.height()
                                );
                                player
                                    .player
                                    .lock()
                                    .unwrap()
                                    .set_viewport_dimensions(viewport_dimensions(&app, window));
                                needs_redraw = true;
                            }
                        }
                        MainEvent::Resume { .. } => {
                            if let (Some(player), Some(window)) =
                                (playerbox.as_ref(), native_window.as_ref())
                            {
                                // [NA] For some reason we can get negative sizes during a resume...
                                if window.width() > 0 && window.height() > 0 {
                                    recreate_surface(&mut player.player.lock().unwrap(), window);
                                }
                            }
                        }
                        MainEvent::InitWindow { .. } => {
                            native_window = app.native_window();
                            if let Some(window) = native_window.as_ref() {
                                log::info!(
                                    "Init window: {} x {} (is existing: {})",
                                    window.width(),
                                    window.height(),
                                    playerbox.is_some()
                                );

                                if let Some(activeplayer) = &playerbox {
                                    let mut player_lock = activeplayer.player.lock().unwrap();
                                    recreate_surface(&mut player_lock, window);
                                    player_lock.set_is_playing(true);
                                } else {
                                    playerbox = create_player(&app, window, &sender, &startup);
                                    last_frame_time = Instant::now();
                                    next_frame_time = Some(Instant::now());
                                }
                            }
                        }
                        MainEvent::TerminateWindow { .. } => {
                            if let Some(player) = playerbox.as_ref() {
                                let mut player_lock = player.player.lock().unwrap();
                                player_lock.set_is_playing(false);
                                player_lock.flush_shared_objects();
                            }
                            native_window = None;
                        }
                        MainEvent::InputAvailable => {
                            if let Ok(mut inputs) = app.input_events_iter() {
                                while inputs.next(|input| {
                                    let Some(player) = playerbox.as_ref() else {
                                        return InputStatus::Unhandled;
                                    };
                                    let status = match input {
                                        InputEvent::MotionEvent(event) => {
                                            match native_window.as_ref() {
                                                Some(window) => {
                                                    handle_motion_event(&app, player, window, event)
                                                }
                                                None => InputStatus::Unhandled,
                                            }
                                        }
                                        InputEvent::KeyEvent(event) => {
                                            handle_key_event(&app, player, event)
                                        }
                                        InputEvent::TextEvent(state) => {
                                            handle_soft_keyboard_text(
                                                player,
                                                &mut ime,
                                                &state.text,
                                            );
                                            InputStatus::Handled
                                        }
                                        InputEvent::TextAction(action) => {
                                            if !matches!(
                                                action,
                                                TextInputAction::None
                                                    | TextInputAction::Unspecified
                                            ) {
                                                tap_enter(&mut player.player.lock().unwrap());
                                            }
                                            InputStatus::Handled
                                        }
                                        _ => InputStatus::Unhandled,
                                    };
                                    if status == InputStatus::Handled {
                                        needs_redraw = true;
                                    }
                                    status
                                }) {}
                            }
                        }
                        _ => {} // Something else happened but it's probably not important for now.
                    },
                    PollEvent::Wake => {} // A task tried to wake us, we'll recv it below
                    PollEvent::Timeout => {} // No events happened, we'll tick as normal below
                    _ => {}               // Unknown future event
                }
            },
        );

        // Only handle the events that are already queued: running a task can queue new ones.
        let events: Vec<RuffleEvent> = receiver.try_iter().collect();
        for event in events {
            match event {
                RuffleEvent::TaskPoll(task) => {
                    // Only run the task if it matches our current player;
                    // otherwise it is stale, and should be cancelled (which
                    // happens implicitly on drop).
                    if let Some(player) = playerbox.as_ref() {
                        if *task.0.metadata() == player.id {
                            task.0.run();
                        }
                    }
                }
                RuffleEvent::VirtualKeyEvent {
                    down,
                    key_descriptor,
                } => {
                    if let Some(player) = playerbox.as_ref() {
                        let mut player = player.player.lock().unwrap();
                        if down {
                            player.handle_event(PlayerEvent::KeyDown {
                                key: key_descriptor,
                            });
                            if let LogicalKey::Character(codepoint) = key_descriptor.logical_key {
                                player.handle_event(PlayerEvent::TextInput { codepoint });
                            }
                        } else {
                            player.handle_event(PlayerEvent::KeyUp {
                                key: key_descriptor,
                            });
                        }
                    }
                }
                RuffleEvent::RunContextMenuCallback(index) => {
                    if let Some(player) = playerbox.as_ref() {
                        player
                            .player
                            .lock()
                            .unwrap()
                            .run_context_menu_callback(index);
                    }
                }
                RuffleEvent::ClearContextMenu => {
                    if let Some(player) = playerbox.as_ref() {
                        player.player.lock().unwrap().clear_custom_menu_items();
                    }
                }
                RuffleEvent::RequestContextMenu => {
                    if let Some(player) = playerbox.as_ref() {
                        let items = player.player.lock().unwrap().prepare_context_menu();
                        if let Err(e) = with_activity(&app, |env, activity| {
                            JavaInterface::show_context_menu(env, activity, &items)
                        }) {
                            log::error!("Couldn't show the context menu: {e}");
                        }
                    }
                }
                RuffleEvent::RootMovieLoaded => {
                    if let Err(e) = with_activity(&app, JavaInterface::on_movie_loaded) {
                        log::error!("Couldn't hide the loading indicator: {e}");
                    }
                }
                RuffleEvent::SoftKeyboard(true) | RuffleEvent::ShowSoftKeyboard => {
                    if !soft_keyboard_open {
                        // Start with an empty text; see `ImeTextTracker`.
                        ime.reset();
                        app.set_text_input_state(TextInputState {
                            text: String::new(),
                            selection: TextSpan { start: 0, end: 0 },
                            compose_region: None,
                        });
                        soft_keyboard_open = true;
                    }
                    app.show_soft_input(false);
                }
                RuffleEvent::SoftKeyboard(false) => {
                    soft_keyboard_open = false;
                    app.hide_soft_input(false);
                }
            }
        }

        let new_time = Instant::now();
        let dt = new_time.duration_since(last_frame_time).as_micros();
        if dt > 0 {
            last_frame_time = new_time;
            if let Some(player) = playerbox.as_ref() {
                if let Ok(mut player) = player.player.lock() {
                    player.tick(FloatDuration::from_millis(dt as f64 / 1000.0));
                    next_frame_time = Some(new_time + player.time_til_next_frame());
                    needs_redraw = player.needs_render();
                    if let Some(audio) =
                        <dyn Any>::downcast_mut::<AndroidAudioBackend>(player.audio_mut())
                    {
                        audio.recreate_stream_if_needed();
                    }
                }
            } else {
                next_frame_time = None;
            }
        }

        if needs_redraw && native_window.is_some() {
            if let Some(player) = playerbox.as_ref() {
                if let Ok(mut player) = player.player.lock() {
                    player.render();
                }
            }
        }
    }

    let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
        // Ensure that we take the EventSender back, or we'll leak it
        let _: jni::errors::Result<EventSender> =
            unsafe { env.take_rust_field(&*activity, jni_str!("eventLoopHandle")) };
        Ok(())
    });
}

/// Sends an event from Java to the event loop.
fn send_event_from_java(
    env: &mut Env,
    this: &JObject,
    event: RuffleEvent,
) -> jni::errors::Result<()> {
    let sender: MutexGuard<EventSender> =
        unsafe { env.get_rust_field(this, jni_str!("eventLoopHandle")) }?;
    sender.send(event);
    Ok(())
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_keydown<'local>(
    mut unowned_env: EnvUnowned<'local>,
    this: JObject<'local>,
    key_tag: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let tag = key_tag.try_to_string(env)?;
            if let Some(desc) = key_tag_to_key_descriptor(&tag) {
                send_event_from_java(
                    env,
                    &this,
                    RuffleEvent::VirtualKeyEvent {
                        down: true,
                        key_descriptor: desc,
                    },
                )?;
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_keyup<'local>(
    mut unowned_env: EnvUnowned<'local>,
    this: JObject<'local>,
    key_tag: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let tag = key_tag.try_to_string(env)?;
            if let Some(desc) = key_tag_to_key_descriptor(&tag) {
                send_event_from_java(
                    env,
                    &this,
                    RuffleEvent::VirtualKeyEvent {
                        down: false,
                        key_descriptor: desc,
                    },
                )?;
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Attaches the current thread to the JVM and runs `f` with the JNI environment and the activity.
///
/// Note: The activity can't be taken from `ndk_context`, because since android-activity 0.6.1,
/// that holds the `Application` instead.
pub fn with_activity<T>(
    app: &AndroidApp,
    f: impl FnOnce(&mut Env, &JObject) -> T,
) -> jni::errors::Result<T> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };

    // This is a global reference, so it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;

    vm.attach_current_thread(|env| {
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
        Ok(f(env, &activity))
    })
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_requestContextMenu<'local>(
    mut unowned_env: EnvUnowned<'local>,
    this: JObject<'local>,
) {
    unowned_env
        .with_env(|env| send_event_from_java(env, &this, RuffleEvent::RequestContextMenu))
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_runContextMenuCallback<'local>(
    mut unowned_env: EnvUnowned<'local>,
    this: JObject<'local>,
    index: jint,
) {
    unowned_env
        .with_env(|env| {
            send_event_from_java(
                env,
                &this,
                RuffleEvent::RunContextMenuCallback(index as usize),
            )
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_clearContextMenu<'local>(
    mut unowned_env: EnvUnowned<'local>,
    this: JObject<'local>,
) {
    unowned_env
        .with_env(|env| send_event_from_java(env, &this, RuffleEvent::ClearContextMenu))
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_showSoftKeyboard<'local>(
    mut unowned_env: EnvUnowned<'local>,
    this: JObject<'local>,
) {
    unowned_env
        .with_env(|env| send_event_from_java(env, &this, RuffleEvent::ShowSoftKeyboard))
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn Java_rs_ruffle_PlayerActivity_nativeInit<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
    crash_callback: JObject<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> { native_init(env, &class, &crash_callback) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

fn native_init(env: &mut Env, class: &JClass, crash_callback: &JObject) -> jni::errors::Result<()> {
    let crash_callback = env.new_global_ref(crash_callback)?;
    let jvm = env.get_java_vm()?;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("ruffle")
            .with_filter(
                android_logger::FilterBuilder::new()
                    .parse("warn,ruffle=info")
                    .build(),
            ),
    );

    panic::set_hook(Box::new(move |info| {
        let backtrace = Backtrace::new();
        let thread = thread::current();
        let thread = thread.name().unwrap_or("<unnamed>");
        let message = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };

        let full = match info.location() {
            Some(location) => format!(
                "thread '{}' panicked at '{}': {}:{}\n{:?}",
                thread,
                message,
                location.file(),
                location.line(),
                backtrace
            ),
            None => format!(
                "thread '{}' panicked at '{}'\n{:?}",
                thread, message, backtrace
            ),
        };
        log::error!(target: "panic","{}", full);

        // Any exception already pending on this thread is stashed by the attachment while the
        // callback runs, and re-thrown afterwards, so java will still discover it on their own.
        let result = jvm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let java_message = env.new_string(full)?;
            env.call_method(
                &*crash_callback,
                jni_str!("onCrash"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[(&java_message).into()],
            )?;
            Ok(())
        });
        if let Err(e) = result {
            log::error!(target: "panic", "Failed to report crash to java: {}", e);
        }
    }));

    JavaInterface::init(env, class);
    Ok(())
}

fn get_loc_in_window(app: &AndroidApp) -> (i32, i32) {
    with_activity(app, JavaInterface::get_loc_in_window).unwrap_or((0, 0))
}

fn get_view_size(app: &AndroidApp) -> Result<(i32, i32), Box<dyn std::error::Error>> {
    let size = with_activity(app, |env, activity| {
        let width = JavaInterface::get_surface_width(env, activity);
        let height = JavaInterface::get_surface_height(env, activity);
        (width, height)
    })?;

    Ok(size)
}

#[no_mangle]
fn android_main(app: AndroidApp) {
    log::info!("Starting android_main...");
    // Must happen before any reqwest client is built (e.g. by the navigator backend).
    let _ = rustls::crypto::ring::default_provider().install_default();
    run(app);
}
