# Patched GameActivity sources

The `android-activity` crate builds the native half of `GameActivity` from C/C++ sources it
bundles (a copy of Google's [android-games-sdk](https://github.com/rust-mobile/android-games-sdk)).
Its build script compiles the `src` files from the directory in the `ANDROID_GAMES_SDK`
environment variable instead, if set. `.cargo/config.toml` points it here.

These are the `src` files of `android-activity` 0.6.1, with one change in `GameActivity.cpp`
(marked with `ruffle-android:` comments): `Configuration.getLocales()` and `android.os.LocaleList`
only exist since Android 7.0 (API 24), and looking them up made the app abort on startup on
older versions. There, the single `Configuration.locale` is used instead.

All other files are unmodified copies; they are only here because the build script takes all
`src` files from the same directory. The headers still come from the crate.

**When updating `android-activity`**, replace these files with the ones from the new version
(`~/.cargo/registry/src/*/android-activity-<version>/android-games-sdk/`) and re-apply the
patch, or the Rust and C++ halves may not match.

License: Apache License 2.0 (see the headers of the files), like the original.
