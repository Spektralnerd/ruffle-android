use jni::objects::{
    JByteArray, JClass, JMethodID, JObject, JObjectArray, JString, JValue, ReleaseMode,
};
use jni::signature::{Primitive, ReturnType};
use jni::{jni_sig, jni_str, Env};
use ruffle_core::ContextMenuItem;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Handles to various items on the Java `PlayerActivity` class.
/// This is statically initialized once at startup, via the Java method `nativeInit()`.
/// This avoids needing to pay the lookup and validation penalty for every single call back into Java,
/// which can be a significant cost.
pub struct JavaInterface {
    get_surface_width: JMethodID,
    get_surface_height: JMethodID,
    show_context_menu: JMethodID,
    get_swf_bytes: JMethodID,
    get_swf_uri: JMethodID,
    get_trace_output: JMethodID,
    get_android_data_storage_dir: JMethodID,
    get_swf_display_name: JMethodID,
    on_movie_loaded: JMethodID,
    show_error: JMethodID,
    show_message: JMethodID,
    get_clipboard_text: JMethodID,
    set_clipboard_text: JMethodID,
}

static JAVA_INTERFACE: OnceLock<JavaInterface> = OnceLock::new();

impl JavaInterface {
    pub fn get_surface_width(env: &mut Env, this: &JObject) -> i32 {
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().get_surface_width,
                ReturnType::Primitive(Primitive::Int),
                &[],
            )
        };
        result
            .expect("getSurfaceWidth() must never throw")
            .i()
            .unwrap()
    }

    pub fn get_surface_height(env: &mut Env, this: &JObject) -> i32 {
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().get_surface_height,
                ReturnType::Primitive(Primitive::Int),
                &[],
            )
        };
        result
            .expect("getSurfaceHeight() must never throw")
            .i()
            .unwrap()
    }

    pub fn show_context_menu(env: &mut Env, this: &JObject, items: &[ContextMenuItem]) {
        let arr = JObjectArray::<JString>::new(env, items.len(), JString::null()).unwrap();
        for (i, e) in items.iter().enumerate() {
            let s = env
                .new_string(format!(
                    "{} {} {} {}",
                    e.enabled, e.separator_before, e.checked, e.caption
                ))
                .unwrap();
            arr.set_element(env, i, &s).unwrap();
        }
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().show_context_menu,
                ReturnType::Primitive(Primitive::Void),
                &[JValue::Object(arr.as_ref()).as_jni()],
            )
        };
        result.expect("showContextMenu() must never throw");
    }

    pub fn get_swf_bytes(env: &mut Env, this: &JObject) -> Option<Vec<u8>> {
        let result = unsafe {
            env.call_method_unchecked(this, Self::get().get_swf_bytes, ReturnType::Array, &[])
        };
        let object = result.expect("getSwfBytes() must never throw").l().unwrap();
        if object.is_null() {
            return None;
        }

        let arr = unsafe { JByteArray::from_raw(env, object.into_raw()) };
        let elements = unsafe { arr.get_elements(env, ReleaseMode::NoCopyBack).unwrap() };
        let data =
            unsafe { std::slice::from_raw_parts(elements.as_ptr() as *mut u8, elements.len()) };
        Some(data.to_vec())
    }

    pub fn get_swf_uri(env: &mut Env, this: &JObject) -> String {
        let result = unsafe {
            env.call_method_unchecked(this, Self::get().get_swf_uri, ReturnType::Object, &[])
        };
        let object = result.expect("getSwfUri() must never throw").l().unwrap();
        if object.is_null() {
            return Default::default();
        }
        let string_object = unsafe { env.as_cast_unchecked::<JString>(&object) };
        string_object.try_to_string(env).unwrap()
    }

    pub fn get_trace_output(env: &mut Env, this: &JObject) -> Option<PathBuf> {
        let result = unsafe {
            env.call_method_unchecked(this, Self::get().get_trace_output, ReturnType::Object, &[])
        };
        let object = result
            .expect("getTraceOutput() must never throw")
            .l()
            .unwrap();
        if object.is_null() {
            return None;
        }
        let string_object = unsafe { env.as_cast_unchecked::<JString>(&object) };
        let url = string_object.try_to_string(env).unwrap();
        Some(url.into())
    }

    pub fn get_android_data_storage_dir(env: &mut Env, this: &JObject) -> PathBuf {
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().get_android_data_storage_dir,
                ReturnType::Object,
                &[],
            )
        };
        let object = result
            .expect("getAndroidDataStorageDir() must never throw")
            .l()
            .unwrap();
        let string_object = unsafe { env.as_cast_unchecked::<JString>(&object) };
        let path = string_object.try_to_string(env).unwrap();
        PathBuf::from(path)
    }

    /// The file name of the movie, if it was opened from a `content://` URI.
    pub fn get_swf_display_name(env: &mut Env, this: &JObject) -> Option<String> {
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().get_swf_display_name,
                ReturnType::Object,
                &[],
            )
        };
        let object = result
            .expect("getSwfDisplayName() must never throw")
            .l()
            .unwrap();
        if object.is_null() {
            return None;
        }
        let string_object = unsafe { env.as_cast_unchecked::<JString>(&object) };
        string_object.try_to_string(env).ok()
    }

    /// Hides the loading indicator.
    pub fn on_movie_loaded(env: &mut Env, this: &JObject) {
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().on_movie_loaded,
                ReturnType::Primitive(Primitive::Void),
                &[],
            )
        };
        result.expect("onMovieLoaded() must never throw");
    }

    /// Shows an error dialog that closes the player. `kind` is a `crate::ui::ErrorKind`.
    pub fn show_error(env: &mut Env, this: &JObject, kind: i32, details: &str) {
        let details = env.new_string(details).unwrap();
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().show_error,
                ReturnType::Primitive(Primitive::Void),
                &[
                    JValue::Int(kind).as_jni(),
                    JValue::Object(details.as_ref()).as_jni(),
                ],
            )
        };
        result.expect("showError() must never throw");
    }

    /// Shows a message from the movie in a dialog.
    pub fn show_message(env: &mut Env, this: &JObject, message: &str) {
        let message = env.new_string(message).unwrap();
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().show_message,
                ReturnType::Primitive(Primitive::Void),
                &[JValue::Object(message.as_ref()).as_jni()],
            )
        };
        result.expect("showMessage() must never throw");
    }

    /// The text in the clipboard, or an empty string.
    pub fn get_clipboard_text(env: &mut Env, this: &JObject) -> String {
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().get_clipboard_text,
                ReturnType::Object,
                &[],
            )
        };
        let object = result
            .expect("getClipboardText() must never throw")
            .l()
            .unwrap();
        if object.is_null() {
            return String::new();
        }
        let string_object = unsafe { env.as_cast_unchecked::<JString>(&object) };
        string_object.try_to_string(env).unwrap_or_default()
    }

    pub fn set_clipboard_text(env: &mut Env, this: &JObject, text: &str) {
        let text = env.new_string(text).unwrap();
        let result = unsafe {
            env.call_method_unchecked(
                this,
                Self::get().set_clipboard_text,
                ReturnType::Primitive(Primitive::Void),
                &[JValue::Object(text.as_ref()).as_jni()],
            )
        };
        result.expect("setClipboardText() must never throw");
    }

    pub fn get() -> &'static JavaInterface {
        JAVA_INTERFACE
            .get()
            .expect("Java interface must have been created via nativeInit()")
    }

    pub fn init(env: &mut Env, class: &JClass) {
        let _ = JAVA_INTERFACE.set(JavaInterface {
            get_surface_width: env
                .get_method_id(class, jni_str!("getSurfaceWidth"), jni_sig!("()I"))
                .expect("getSurfaceWidth must exist"),
            get_surface_height: env
                .get_method_id(class, jni_str!("getSurfaceHeight"), jni_sig!("()I"))
                .expect("getSurfaceHeight must exist"),
            show_context_menu: env
                .get_method_id(
                    class,
                    jni_str!("showContextMenu"),
                    jni_sig!("([Ljava/lang/String;)V"),
                )
                .expect("showContextMenu must exist"),
            get_swf_bytes: env
                .get_method_id(class, jni_str!("getSwfBytes"), jni_sig!("()[B"))
                .expect("getSwfBytes must exist"),
            get_swf_uri: env
                .get_method_id(
                    class,
                    jni_str!("getSwfUri"),
                    jni_sig!("()Ljava/lang/String;"),
                )
                .expect("getSwfUri must exist"),
            get_trace_output: env
                .get_method_id(
                    class,
                    jni_str!("getTraceOutput"),
                    jni_sig!("()Ljava/lang/String;"),
                )
                .expect("getTraceOutput must exist"),
            get_android_data_storage_dir: env
                .get_method_id(
                    class,
                    jni_str!("getAndroidDataStorageDir"),
                    jni_sig!("()Ljava/lang/String;"),
                )
                .expect("getAndroidDataStorageDir must exist"),
            get_swf_display_name: env
                .get_method_id(
                    class,
                    jni_str!("getSwfDisplayName"),
                    jni_sig!("()Ljava/lang/String;"),
                )
                .expect("getSwfDisplayName must exist"),
            on_movie_loaded: env
                .get_method_id(class, jni_str!("onMovieLoaded"), jni_sig!("()V"))
                .expect("onMovieLoaded must exist"),
            show_error: env
                .get_method_id(
                    class,
                    jni_str!("showError"),
                    jni_sig!("(ILjava/lang/String;)V"),
                )
                .expect("showError must exist"),
            show_message: env
                .get_method_id(
                    class,
                    jni_str!("showMessage"),
                    jni_sig!("(Ljava/lang/String;)V"),
                )
                .expect("showMessage must exist"),
            get_clipboard_text: env
                .get_method_id(
                    class,
                    jni_str!("getClipboardText"),
                    jni_sig!("()Ljava/lang/String;"),
                )
                .expect("getClipboardText must exist"),
            set_clipboard_text: env
                .get_method_id(
                    class,
                    jni_str!("setClipboardText"),
                    jni_sig!("(Ljava/lang/String;)V"),
                )
                .expect("setClipboardText must exist"),
        });
    }
}
