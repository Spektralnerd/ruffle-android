This is a native Android application for [Ruffle](https://ruffle.rs).

It is in a very early stage.

# Prebuilt APKs

[<img src="https://fdroid.gitlab.io/artwork/badge/get-it-on.png"
     alt="Get it on F-Droid"
     height="75">](https://f-droid.org/packages/rs.ruffle/)

The latest release [(here)](https://github.com/ruffle-rs/ruffle-android/releases) should have a few `.apk` files uploaded as assets.

You can try this app by downloading and installing one of those.

- **For the vast majority of modern phones, tablets, single board computers, and small game consoles, you'll need the `arm64-v8a` version.**

- The `armeabi-v7a` version is for older, 32-bit ARM stuff.

- The `x86_64` version is for some rare Intel/Microsoft tablets and/or for Chromebooks, and/or for running on a PC on Android-x86 or in Waydroid or similar.

- The `x86` version is there mostly just for completeness.

- The `universal` version should work on all 4 of the above architectures, but it's _huge_.

# Supported Android versions

Android 5.0 (API 21) and newer, on devices with OpenGL ES 3.0 graphics (which most devices
from 2013 on have). Ruffle's renderer needs OpenGL ES 3.0, so it can't run on older devices, nor
on Android versions before 5.0. Sound is played with AAudio on Android 8.1 and newer, and with
OpenSL ES before that.

# Controls

- Touching the screen acts as the (left) mouse button. A mouse or touchpad also works, including
  the right button and the wheel.
- **⌨** shows or hides the on-screen keys (digits, letters, arrows, Space, Ctrl, Alt). They are
  hidden in landscape at first.
- **Aa** opens the system's keyboard, to type text or to play with it. It also opens by itself
  when an editable text field in the movie is tapped.
- **▤** opens the context menu of the movie, with an "Exit" item. Back closes the player too.
- Physical keyboards work as usual.
- Gamepads: the D-pad and the left stick act as the arrow keys, and the buttons are pressed as
  these keys:

  | Button | Key   |    | Button | Key    |
  |--------|-------|----|--------|--------|
  | A      | Space |    | L1     | Shift  |
  | B      | X     |    | R1     | Ctrl   |
  | X      | Z     |    | L2     | Q      |
  | Y      | C     |    | R2     | E      |
  | Start  | Enter |    | Select | Escape |

Saved games (SharedObjects) of files opened with the file picker are kept per file name, so they
are found again when the same file is opened later, also from the list of recent files.

# Building from source

Please see [CONTRIBUTING.md](CONTRIBUTING.md#building-from-source) for details about how to build this repository yourself.

---

# TODO

In no particular order:

- [X] Ability to show the built-in virtual keyboard (softinput), for text input
  - Opens when an editable text field gets the focus, and with the "Aa" button
- [X] Controller/Gamepad input?
  - Mapped to key presses (see [Controls](#controls)), no virtual mouse pointer (yet)
- [ ] Own custom keyboard overlay, maybe even per-content configs
  - Not an overlay, and not per-content, but custom keyboard is there
- [X] Error/panic handling
  - Movies that can't be loaded, invalid files and unsupported graphics are explained in a
    dialog instead of crashing, and the crash screen can copy or share its report
- [X] Loading "animation" (spinner)
- [X] Alternative audio backend (OpenSL ES) for Android < 8
  - AAudio is loaded at runtime now, so the app also starts on older versions: Android 5.0+
- [X] Proper storage backend?
  - Saves no longer depend on the `content://` URL a file was opened with, which can change,
    and they are written when the app goes to the background
- [X] Resolve design glitches/styling/theming (immersive mode, window insets for holes/notches/corners)
  - The movie and the buttons stay clear of notches and camera holes, and above the soft
    keyboard; the system bars are hidden again after dialogs. Rounded corners aren't handled.
- [ ] Publish to various app stores, maybe automatically?
- [ ] Bundle demo animations/games
- [ ] Add ability to load content from well known online collections? (well maybe not z0r... unless?)
- [X] History, favorites, other flair...?
  - Recently opened movies on the start screen, with favorites (★) kept at the top

### DONE:

- [X] Clean up ~everything
- [X] Cross-platform build instructions?
  - I think gradle should take care of it now
- [X] UI backend (context menu)
  - Context menu works
- [X] Logging?
- [X] Navigator backend (fetch, open browser)
  - Opening links works at least
- [X] Touch/mouse input
- [X] Keyboard input: only with physical keyboard connected or through `scrcpy`
  - This was needed: https://github.com/rust-windowing/winit/pull/2226
- [X] Split into a separate repo
- [X] Add ability to Open SWF by entered/pasted URL (or even directly from clipboard)
  - No direct clipboard open, but easy to paste into the text field...
- [X] Unglitchify rendering: scale, center and letterbox the content properly
- [ ] Ask CPAL/Oboe to open a "media" type output stream instead of a "call" one
  - so the right volume slider controls it, and it uses the loud(er)speaker
  - -> solved by switching to a direct AAudio (ndk-audio) backend
- [X] Add building this to CI, at least to the release workflow
  - This repo has its own CI setup, which builds APKs
- [X] Simplify build process (hook cargo-apk into gradle, drop cargo-apk?)
  - ~cargo-apk is fine, but is only used to detect the SDK/NDK environment and run Cargo in it, and not to build an APK.~
  - actually solved by switching to `cargo-ndk` and the corresponding Gradle plugin
- [X] Somehow filter files to be picked to .swf
  - How well this works depends on the file picker, but it "should work most of the time"
- [X] Unglitchify audio volume (buttons unresponsive?)
  - (pending: https://github.com/rust-windowing/winit/pull/1919)
  - actually solved by switching to GameActivity instead
- [ ] Register Ruffle to open .swf files
  - How well this works depends on the application opening the file, but it "should work most of the time"
- [X] Figure out why videos are not playing (could be a seeking issue)
  - The video decoder features weren't enabled on `ruffle_core`...
- [X] Sign the APK
  - Using a very simple key for now, with just my name in it
- [X] Support for 32-bit ARM phones
  - Untested, but should work in theory
- [X] Support for x86(_64) tablets?
  - Sorted out
- [X] Consider not building the intermediate .apk just for the shared libraries
  - Figured out, no intermediate .apk any more, only native libs built
- [ ] Unbreak the regular build on CI
  - No longer relevant after the repo split
- [ ] Clean up commit history of the branch
  - No longer relevant after the repo split
