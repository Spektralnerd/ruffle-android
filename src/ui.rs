use crate::custom_event::RuffleEvent;
use crate::java::JavaInterface;
use crate::{with_activity, EventSender};
use android_activity::AndroidApp;
use ruffle_core::backend::ui::{
    DialogResultFuture, FileDialogResult, FileFilter, FontDefinition, FullscreenError,
    LanguageIdentifier, MouseCursor, MultiDialogResultFuture, MultiFileDialogResult, UiBackend,
    US_ENGLISH,
};
use ruffle_core::font::FontQuery;
use std::cell::Cell;
use url::Url;

/// The kinds of errors `PlayerActivity.showError()` knows how to explain to the user.
/// Keep in sync with the constants in `PlayerActivity`.
#[derive(Copy, Clone, Debug)]
#[repr(i32)]
pub enum ErrorKind {
    /// The movie could not be downloaded or read.
    MovieLoadFailed = 0,
    /// The file is not a (valid) SWF.
    InvalidMovie = 1,
    /// The graphics (OpenGL ES 3) could not be initialized.
    RendererFailed = 2,
}

/// Shows an error dialog, which closes the player when dismissed.
pub fn show_error(app: &AndroidApp, kind: ErrorKind, details: &str) {
    log::error!("{kind:?}: {details}");
    let result = with_activity(app, |env, activity| {
        JavaInterface::show_error(env, activity, kind as i32, details)
    });
    if let Err(e) = result {
        log::error!("Couldn't show the error to the user: {e}");
    }
}

/// The language of the device, as configured by the user.
fn system_language(app: &AndroidApp) -> LanguageIdentifier {
    let config = app.config();
    let tag = match (config.language(), config.country()) {
        (Some(language), Some(country)) if !country.is_empty() => {
            format!("{language}-{country}")
        }
        (Some(language), _) => language,
        _ => return US_ENGLISH.clone(),
    };
    tag.parse().unwrap_or_else(|_| US_ENGLISH.clone())
}

/// Connects the parts of Ruffle that need the user interface (dialogs, the soft keyboard,
/// the clipboard) to the Android activity.
///
/// Ruffle calls these methods on the player thread while it holds the player lock, so they
/// must never lock the player themselves. Anything slow or touching Android views is handed
/// over to the UI thread (by the Kotlin side) or to the event loop.
pub struct AndroidUiBackend {
    app: AndroidApp,
    event_sender: EventSender,
    language: LanguageIdentifier,
    mouse_visible: bool,
    /// Whether the soft keyboard is open for a focused text field.
    keyboard_requested: Cell<bool>,
}

impl AndroidUiBackend {
    pub fn new(app: AndroidApp, event_sender: EventSender) -> Self {
        let language = system_language(&app);
        Self {
            app,
            event_sender,
            language,
            mouse_visible: true,
            keyboard_requested: Cell::new(false),
        }
    }

    /// The user closed the soft keyboard, so it has to be opened again the next time a text
    /// field asks for it (e.g. when it is tapped again).
    pub fn soft_keyboard_hidden(&self) {
        self.keyboard_requested.set(false);
    }
}

impl UiBackend for AndroidUiBackend {
    fn mouse_visible(&self) -> bool {
        self.mouse_visible
    }

    fn set_mouse_visible(&mut self, visible: bool) {
        self.mouse_visible = visible;
    }

    fn set_mouse_cursor(&mut self, _cursor: MouseCursor) {}

    fn clipboard_content(&mut self) -> String {
        with_activity(&self.app, JavaInterface::get_clipboard_text).unwrap_or_default()
    }

    fn set_clipboard_content(&mut self, content: String) {
        let result = with_activity(&self.app, |env, activity| {
            JavaInterface::set_clipboard_text(env, activity, &content)
        });
        if let Err(e) = result {
            log::warn!("Couldn't set the clipboard: {e}");
        }
    }

    fn set_fullscreen(&mut self, _is_full: bool) -> Result<(), FullscreenError> {
        // The player always fills the whole screen already.
        Ok(())
    }

    fn display_root_movie_download_failed_message(&self, invalid_swf: bool, fetched_error: String) {
        let kind = if invalid_swf {
            ErrorKind::InvalidMovie
        } else {
            ErrorKind::MovieLoadFailed
        };
        show_error(&self.app, kind, &fetched_error);
    }

    fn message(&self, message: &str) {
        let result = with_activity(&self.app, |env, activity| {
            JavaInterface::show_message(env, activity, message)
        });
        if let Err(e) = result {
            log::warn!("Couldn't show message \"{message}\": {e}");
        }
    }

    fn open_virtual_keyboard(&self) {
        // Ruffle asks again on every focus update, even without a change (which some movies
        // cause every frame), but (re)opening the keyboard restarts its input.
        if !self.keyboard_requested.replace(true) {
            self.event_sender.send(RuffleEvent::SoftKeyboard(true));
        }
    }

    fn close_virtual_keyboard(&self) {
        // This is called on most clicks, so only act if we were the ones who opened it.
        // A keyboard opened with the toolbar button stays open (for games played with it).
        if self.keyboard_requested.replace(false) {
            self.event_sender.send(RuffleEvent::SoftKeyboard(false));
        }
    }

    fn language(&self) -> LanguageIdentifier {
        self.language.clone()
    }

    fn display_unsupported_video(&self, url: Url) {
        if url.scheme() == "javascript" {
            return;
        }
        // Let a video player app (or the browser) play it instead.
        let _ = webbrowser::open(url.as_str());
    }

    fn load_device_font(&self, _query: &FontQuery, _register: &mut dyn FnMut(FontDefinition)) {}

    fn sort_device_fonts(
        &self,
        _query: &FontQuery,
        _register: &mut dyn FnMut(FontDefinition),
    ) -> Vec<FontQuery> {
        Vec::new()
    }

    fn display_file_open_dialog(
        &mut self,
        _filters: Vec<FileFilter>,
    ) -> Option<DialogResultFuture> {
        Some(Box::pin(async { Ok(FileDialogResult::Canceled) }))
    }

    fn display_file_open_dialog_multiple(
        &mut self,
        _filters: Vec<FileFilter>,
    ) -> Option<MultiDialogResultFuture> {
        Some(Box::pin(async { Ok(MultiFileDialogResult::Canceled) }))
    }

    fn display_file_save_dialog(
        &mut self,
        _file_name: String,
        _title: String,
    ) -> Option<DialogResultFuture> {
        None
    }

    fn close_file_dialog(&mut self) {}
}
