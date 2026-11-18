//! Minimal helpers for loading system libraries at runtime with `dlopen`/`dlsym`.
//!
//! The audio libraries are loaded like this instead of being linked, because linking
//! adds a `DT_NEEDED` entry, and then this whole library fails to load on any device
//! where the linked library is missing (`libaaudio.so` only exists since Android 8.0).
//! Oboe does the same, for both AAudio and OpenSL ES.
//!
//! The library handles are never `dlclose`d (like in Oboe): the libraries are only
//! loaded once per process (the results are kept in `OnceLock` statics), and an audio
//! callback must never be able to run code from an unloaded library.

use std::ffi::{c_void, CStr};
use std::mem::size_of;
use std::ptr::NonNull;

/// The handle of a library loaded with `dlopen`, never closed.
#[derive(Clone, Copy)]
pub struct Library(NonNull<c_void>);

fn last_error() -> String {
    // SAFETY: `dlerror` returns either null or a valid C string (thread-local in bionic).
    unsafe {
        let error = libc::dlerror();
        if error.is_null() {
            "unknown error".to_string()
        } else {
            CStr::from_ptr(error).to_string_lossy().into_owned()
        }
    }
}

/// Converts `name`, which must end with a NUL character, to a C string.
fn c_name(name: &str) -> Result<&CStr, String> {
    CStr::from_bytes_with_nul(name.as_bytes())
        .map_err(|_| format!("Invalid symbol or library name: {name:?}"))
}

/// Formats a NUL-terminated name for messages.
fn display_name(name: &str) -> &str {
    name.trim_end_matches('\0')
}

impl Library {
    /// Loads the system library `name` (which must end with a NUL character).
    pub fn open(name: &str) -> Result<Self, String> {
        let c_name = c_name(name)?;
        // SAFETY: `c_name` is a valid C string. `RTLD_NOW` (whose value differs between
        // 32-bit and 64-bit bionic, so the libc constant must be used) resolves all
        // symbols of the library now, like Oboe does.
        let handle = unsafe { libc::dlopen(c_name.as_ptr(), libc::RTLD_NOW) };
        NonNull::new(handle)
            .map(Library)
            .ok_or_else(|| format!("dlopen({}) failed: {}", display_name(name), last_error()))
    }

    /// Looks up the address of the symbol `name` (which must end with a NUL character).
    pub fn symbol(self, name: &str) -> Result<NonNull<c_void>, String> {
        let c_name = c_name(name)?;
        // SAFETY: The handle came from a successful `dlopen` and is never closed,
        // and `c_name` is a valid C string.
        let address = unsafe { libc::dlsym(self.0.as_ptr(), c_name.as_ptr()) };
        NonNull::new(address)
            .ok_or_else(|| format!("dlsym({}) failed: {}", display_name(name), last_error()))
    }

    /// Looks up the function `name` (which must end with a NUL character).
    ///
    /// # Safety
    ///
    /// `F` must be an `unsafe extern "C" fn` pointer type matching the C prototype
    /// of the function.
    pub unsafe fn function<F: Copy>(self, name: &str) -> Result<F, String> {
        if size_of::<F>() != size_of::<*mut c_void>() {
            return Err(format!(
                "{} can't be loaded as a function pointer of this type",
                display_name(name)
            ));
        }
        let address = self.symbol(name)?.as_ptr();
        // SAFETY: `F` is a function pointer type of the same size as a data pointer
        // (true for every Android ABI), and the caller guarantees that it matches.
        Ok(std::mem::transmute_copy::<*mut c_void, F>(&address))
    }
}
