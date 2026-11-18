use android_activity::input::{Keycode, MetaState};
use ruffle_core::events::{
    GamepadButton, KeyCode, KeyDescriptor, KeyLocation, LogicalKey, NamedKey, PhysicalKey,
    TextControlCode,
};
use std::collections::HashMap;

/// Maps an Android key code to the physical key it sits on, and the logical key it produces
/// without any modifiers.
///
/// Every key gets its own physical key where possible: Ruffle tracks pressed keys by their
/// physical key (and location), so sharing one (e.g. `PhysicalKey::Unknown`) between keys
/// would make it drop the key-up event of all but one of them when several are held at once.
fn keycode_to_keys(key_code: Keycode) -> Option<(PhysicalKey, LogicalKey)> {
    use LogicalKey::{Character as C, Named as N};
    use PhysicalKey as P;

    Some(match key_code {
        Keycode::DpadUp => (P::ArrowUp, N(NamedKey::ArrowUp)),
        Keycode::DpadDown => (P::ArrowDown, N(NamedKey::ArrowDown)),
        Keycode::DpadLeft => (P::ArrowLeft, N(NamedKey::ArrowLeft)),
        Keycode::DpadRight => (P::ArrowRight, N(NamedKey::ArrowRight)),
        Keycode::DpadCenter => (P::Enter, N(NamedKey::Enter)),
        Keycode::Keycode0 => (P::Digit0, C('0')),
        Keycode::Keycode1 => (P::Digit1, C('1')),
        Keycode::Keycode2 => (P::Digit2, C('2')),
        Keycode::Keycode3 => (P::Digit3, C('3')),
        Keycode::Keycode4 => (P::Digit4, C('4')),
        Keycode::Keycode5 => (P::Digit5, C('5')),
        Keycode::Keycode6 => (P::Digit6, C('6')),
        Keycode::Keycode7 => (P::Digit7, C('7')),
        Keycode::Keycode8 => (P::Digit8, C('8')),
        Keycode::Keycode9 => (P::Digit9, C('9')),
        Keycode::A => (P::KeyA, C('a')),
        Keycode::B => (P::KeyB, C('b')),
        Keycode::C => (P::KeyC, C('c')),
        Keycode::D => (P::KeyD, C('d')),
        Keycode::E => (P::KeyE, C('e')),
        Keycode::F => (P::KeyF, C('f')),
        Keycode::G => (P::KeyG, C('g')),
        Keycode::H => (P::KeyH, C('h')),
        Keycode::I => (P::KeyI, C('i')),
        Keycode::J => (P::KeyJ, C('j')),
        Keycode::K => (P::KeyK, C('k')),
        Keycode::L => (P::KeyL, C('l')),
        Keycode::M => (P::KeyM, C('m')),
        Keycode::N => (P::KeyN, C('n')),
        Keycode::O => (P::KeyO, C('o')),
        Keycode::P => (P::KeyP, C('p')),
        Keycode::Q => (P::KeyQ, C('q')),
        Keycode::R => (P::KeyR, C('r')),
        Keycode::S => (P::KeyS, C('s')),
        Keycode::T => (P::KeyT, C('t')),
        Keycode::U => (P::KeyU, C('u')),
        Keycode::V => (P::KeyV, C('v')),
        Keycode::W => (P::KeyW, C('w')),
        Keycode::X => (P::KeyX, C('x')),
        Keycode::Y => (P::KeyY, C('y')),
        Keycode::Z => (P::KeyZ, C('z')),
        Keycode::Comma => (P::Comma, C(',')),
        Keycode::Period => (P::Period, C('.')),
        Keycode::AltLeft => (P::AltLeft, N(NamedKey::Alt)),
        Keycode::AltRight => (P::AltRight, N(NamedKey::Alt)),
        Keycode::ShiftLeft => (P::ShiftLeft, N(NamedKey::Shift)),
        Keycode::ShiftRight => (P::ShiftRight, N(NamedKey::Shift)),
        Keycode::CtrlLeft => (P::ControlLeft, N(NamedKey::Control)),
        Keycode::CtrlRight => (P::ControlRight, N(NamedKey::Control)),
        Keycode::MetaLeft => (P::SuperLeft, N(NamedKey::Super)),
        Keycode::MetaRight => (P::SuperRight, N(NamedKey::Super)),
        Keycode::Tab => (P::Tab, N(NamedKey::Tab)),
        Keycode::Space => (P::Space, C(' ')),
        Keycode::Enter => (P::Enter, N(NamedKey::Enter)),
        Keycode::Del => (P::Backspace, N(NamedKey::Backspace)),
        Keycode::ForwardDel => (P::Delete, N(NamedKey::Delete)),
        Keycode::Grave => (P::Backquote, C('`')),
        Keycode::Minus => (P::Minus, C('-')),
        Keycode::Equals => (P::Equal, C('=')),
        Keycode::LeftBracket => (P::BracketLeft, C('[')),
        Keycode::RightBracket => (P::BracketRight, C(']')),
        Keycode::Backslash => (P::Backslash, C('\\')),
        Keycode::Semicolon => (P::Semicolon, C(';')),
        Keycode::Apostrophe => (P::Quote, C('\'')),
        Keycode::Slash => (P::Slash, C('/')),
        Keycode::Plus => (P::Unknown, C('+')),
        Keycode::At => (P::Unknown, C('@')),
        Keycode::Star => (P::Unknown, C('*')),
        Keycode::Pound => (P::Unknown, C('#')),
        Keycode::PageUp => (P::PageUp, N(NamedKey::PageUp)),
        Keycode::PageDown => (P::PageDown, N(NamedKey::PageDown)),
        Keycode::Escape => (P::Escape, N(NamedKey::Escape)),
        Keycode::CapsLock => (P::CapsLock, N(NamedKey::CapsLock)),
        Keycode::ScrollLock => (P::ScrollLock, N(NamedKey::ScrollLock)),
        Keycode::Break => (P::Pause, N(NamedKey::Pause)),
        Keycode::MoveHome => (P::Home, N(NamedKey::Home)),
        Keycode::MoveEnd => (P::End, N(NamedKey::End)),
        Keycode::Insert => (P::Insert, N(NamedKey::Insert)),
        Keycode::Sysrq => (P::PrintScreen, N(NamedKey::PrintScreen)),
        Keycode::F1 => (P::F1, N(NamedKey::F1)),
        Keycode::F2 => (P::F2, N(NamedKey::F2)),
        Keycode::F3 => (P::F3, N(NamedKey::F3)),
        Keycode::F4 => (P::F4, N(NamedKey::F4)),
        Keycode::F5 => (P::F5, N(NamedKey::F5)),
        Keycode::F6 => (P::F6, N(NamedKey::F6)),
        Keycode::F7 => (P::F7, N(NamedKey::F7)),
        Keycode::F8 => (P::F8, N(NamedKey::F8)),
        Keycode::F9 => (P::F9, N(NamedKey::F9)),
        Keycode::F10 => (P::F10, N(NamedKey::F10)),
        Keycode::F11 => (P::F11, N(NamedKey::F11)),
        Keycode::F12 => (P::F12, N(NamedKey::F12)),
        Keycode::NumLock => (P::NumLock, N(NamedKey::NumLock)),
        Keycode::Numpad0 => (P::Numpad0, C('0')),
        Keycode::Numpad1 => (P::Numpad1, C('1')),
        Keycode::Numpad2 => (P::Numpad2, C('2')),
        Keycode::Numpad3 => (P::Numpad3, C('3')),
        Keycode::Numpad4 => (P::Numpad4, C('4')),
        Keycode::Numpad5 => (P::Numpad5, C('5')),
        Keycode::Numpad6 => (P::Numpad6, C('6')),
        Keycode::Numpad7 => (P::Numpad7, C('7')),
        Keycode::Numpad8 => (P::Numpad8, C('8')),
        Keycode::Numpad9 => (P::Numpad9, C('9')),
        Keycode::NumpadDivide => (P::NumpadDivide, C('/')),
        Keycode::NumpadMultiply => (P::NumpadMultiply, C('*')),
        Keycode::NumpadSubtract => (P::NumpadSubtract, C('-')),
        Keycode::NumpadAdd => (P::NumpadAdd, C('+')),
        Keycode::NumpadDot => (P::NumpadDecimal, C('.')),
        Keycode::NumpadComma => (P::NumpadComma, C(',')),
        Keycode::NumpadEnter => (P::NumpadEnter, N(NamedKey::Enter)),
        Keycode::NumpadEquals => (P::Unknown, C('=')),
        _ => return None,
    })
}

fn key_location(key_code: Keycode) -> KeyLocation {
    match key_code {
        Keycode::AltLeft | Keycode::ShiftLeft | Keycode::CtrlLeft | Keycode::MetaLeft => {
            KeyLocation::Left
        }

        Keycode::AltRight | Keycode::ShiftRight | Keycode::CtrlRight | Keycode::MetaRight => {
            KeyLocation::Right
        }

        Keycode::NumLock
        | Keycode::Numpad0
        | Keycode::Numpad1
        | Keycode::Numpad2
        | Keycode::Numpad3
        | Keycode::Numpad4
        | Keycode::Numpad5
        | Keycode::Numpad6
        | Keycode::Numpad7
        | Keycode::Numpad8
        | Keycode::Numpad9
        | Keycode::NumpadDivide
        | Keycode::NumpadMultiply
        | Keycode::NumpadSubtract
        | Keycode::NumpadAdd
        | Keycode::NumpadDot
        | Keycode::NumpadComma
        | Keycode::NumpadEnter
        | Keycode::NumpadEquals
        | Keycode::NumpadLeftParen
        | Keycode::NumpadRightParen => KeyLocation::Numpad,

        _ => KeyLocation::Standard,
    }
}

/// Converts an Android key to a Ruffle key descriptor.
///
/// `character` is the character the key produces with the current modifiers and keyboard
/// layout (from the device's `KeyCharacterMap`), if known. It replaces the unshifted
/// character, so that e.g. Shift+A is reported as 'A', and keys of non-US layouts produce
/// the right characters.
pub fn android_key_to_ruffle_key_descriptor(
    key_code: Keycode,
    character: Option<char>,
) -> Option<KeyDescriptor> {
    let (physical_key, logical_key) = match (keycode_to_keys(key_code), character) {
        (Some((physical_key, LogicalKey::Character(_))), Some(c)) => {
            (physical_key, LogicalKey::Character(c))
        }
        (Some(keys), _) => keys,
        // A key we don't know, but which produces a character on this keyboard.
        (None, Some(c)) => (PhysicalKey::Unknown, LogicalKey::Character(c)),
        (None, None) => return None,
    };

    Some(KeyDescriptor {
        physical_key,
        logical_key,
        key_location: key_location(key_code),
    })
}

/// Text editing commands for keys pressed on a physical keyboard, mirroring Ruffle desktop.
pub fn android_key_to_text_control(key_code: Keycode, meta: MetaState) -> Option<TextControlCode> {
    let shift = meta.shift_on();
    let ctrl = meta.ctrl_on();
    Some(match key_code {
        Keycode::Enter | Keycode::NumpadEnter => TextControlCode::Enter,
        Keycode::A if ctrl => TextControlCode::SelectAll,
        Keycode::C if ctrl => TextControlCode::Copy,
        Keycode::V if ctrl => TextControlCode::Paste,
        Keycode::X if ctrl => TextControlCode::Cut,
        Keycode::Del if ctrl => TextControlCode::BackspaceWord,
        Keycode::Del => TextControlCode::Backspace,
        Keycode::ForwardDel if ctrl => TextControlCode::DeleteWord,
        Keycode::ForwardDel => TextControlCode::Delete,
        Keycode::DpadLeft if ctrl && shift => TextControlCode::SelectLeftWord,
        Keycode::DpadLeft if ctrl => TextControlCode::MoveLeftWord,
        Keycode::DpadLeft if shift => TextControlCode::SelectLeft,
        Keycode::DpadLeft => TextControlCode::MoveLeft,
        Keycode::DpadRight if ctrl && shift => TextControlCode::SelectRightWord,
        Keycode::DpadRight if ctrl => TextControlCode::MoveRightWord,
        Keycode::DpadRight if shift => TextControlCode::SelectRight,
        Keycode::DpadRight => TextControlCode::MoveRight,
        Keycode::MoveHome if ctrl && shift => TextControlCode::SelectLeftDocument,
        Keycode::MoveHome if ctrl => TextControlCode::MoveLeftDocument,
        Keycode::MoveHome if shift => TextControlCode::SelectLeftLine,
        Keycode::MoveHome => TextControlCode::MoveLeftLine,
        Keycode::MoveEnd if ctrl && shift => TextControlCode::SelectRightDocument,
        Keycode::MoveEnd if ctrl => TextControlCode::MoveRightDocument,
        Keycode::MoveEnd if shift => TextControlCode::SelectRightLine,
        Keycode::MoveEnd => TextControlCode::MoveRightLine,
        _ => return None,
    })
}

/// Gamepad buttons, as reported by Android, mapped to Ruffle's (gilrs-style) gamepad buttons.
///
/// The D-pad is not included: it arrives as `KEYCODE_DPAD_*` (shared with TV remotes and
/// keyboards), which already map straight to the arrow keys.
pub fn android_key_to_gamepad_button(key_code: Keycode) -> Option<GamepadButton> {
    Some(match key_code {
        Keycode::ButtonA => GamepadButton::South,
        Keycode::ButtonB => GamepadButton::East,
        Keycode::ButtonX => GamepadButton::West,
        Keycode::ButtonY => GamepadButton::North,
        Keycode::ButtonL1 => GamepadButton::LeftTrigger,
        Keycode::ButtonL2 => GamepadButton::LeftTrigger2,
        Keycode::ButtonR1 => GamepadButton::RightTrigger,
        Keycode::ButtonR2 => GamepadButton::RightTrigger2,
        Keycode::ButtonSelect => GamepadButton::Select,
        Keycode::ButtonStart => GamepadButton::Start,
        _ => return None,
    })
}

/// The keys that gamepad buttons (and the analog stick / hat, as D-pad) are translated to.
/// Ruffle itself has no default for this, and drops gamepad input without a mapping.
///
/// Arrows plus Space, Z, X and C cover the controls of most Flash games.
pub fn default_gamepad_button_mapping() -> HashMap<GamepadButton, KeyCode> {
    HashMap::from([
        (GamepadButton::DPadUp, KeyCode::UP),
        (GamepadButton::DPadDown, KeyCode::DOWN),
        (GamepadButton::DPadLeft, KeyCode::LEFT),
        (GamepadButton::DPadRight, KeyCode::RIGHT),
        (GamepadButton::South, KeyCode::SPACE),
        (GamepadButton::East, KeyCode::X),
        (GamepadButton::West, KeyCode::Z),
        (GamepadButton::North, KeyCode::C),
        (GamepadButton::LeftTrigger, KeyCode::SHIFT),
        (GamepadButton::RightTrigger, KeyCode::CONTROL),
        (GamepadButton::LeftTrigger2, KeyCode::Q),
        (GamepadButton::RightTrigger2, KeyCode::E),
        (GamepadButton::Start, KeyCode::ENTER),
        (GamepadButton::Select, KeyCode::ESCAPE),
    ])
}

/// A key descriptor for a character typed on the soft keyboard (which has no physical keys).
pub fn char_to_key_descriptor(c: char) -> KeyDescriptor {
    let physical_key = match c.to_ascii_lowercase() {
        'a' => PhysicalKey::KeyA,
        'b' => PhysicalKey::KeyB,
        'c' => PhysicalKey::KeyC,
        'd' => PhysicalKey::KeyD,
        'e' => PhysicalKey::KeyE,
        'f' => PhysicalKey::KeyF,
        'g' => PhysicalKey::KeyG,
        'h' => PhysicalKey::KeyH,
        'i' => PhysicalKey::KeyI,
        'j' => PhysicalKey::KeyJ,
        'k' => PhysicalKey::KeyK,
        'l' => PhysicalKey::KeyL,
        'm' => PhysicalKey::KeyM,
        'n' => PhysicalKey::KeyN,
        'o' => PhysicalKey::KeyO,
        'p' => PhysicalKey::KeyP,
        'q' => PhysicalKey::KeyQ,
        'r' => PhysicalKey::KeyR,
        's' => PhysicalKey::KeyS,
        't' => PhysicalKey::KeyT,
        'u' => PhysicalKey::KeyU,
        'v' => PhysicalKey::KeyV,
        'w' => PhysicalKey::KeyW,
        'x' => PhysicalKey::KeyX,
        'y' => PhysicalKey::KeyY,
        'z' => PhysicalKey::KeyZ,
        '0' => PhysicalKey::Digit0,
        '1' => PhysicalKey::Digit1,
        '2' => PhysicalKey::Digit2,
        '3' => PhysicalKey::Digit3,
        '4' => PhysicalKey::Digit4,
        '5' => PhysicalKey::Digit5,
        '6' => PhysicalKey::Digit6,
        '7' => PhysicalKey::Digit7,
        '8' => PhysicalKey::Digit8,
        '9' => PhysicalKey::Digit9,
        ' ' => PhysicalKey::Space,
        _ => PhysicalKey::Unknown,
    };
    KeyDescriptor {
        physical_key,
        logical_key: LogicalKey::Character(c),
        key_location: KeyLocation::Standard,
    }
}

/// A key descriptor for a named key sent by the soft keyboard (Backspace, Enter).
pub fn named_key_descriptor(physical_key: PhysicalKey, named_key: NamedKey) -> KeyDescriptor {
    KeyDescriptor {
        physical_key,
        logical_key: LogicalKey::Named(named_key),
        key_location: KeyLocation::Standard,
    }
}

/// Keys of the on-screen keyboard in the player layout, identified by their `android:tag`.
pub fn key_tag_to_key_descriptor(tag: &str) -> Option<KeyDescriptor> {
    let named = |physical_key, named_key| Some(named_key_descriptor(physical_key, named_key));
    match tag {
        "UP" => named(PhysicalKey::ArrowUp, NamedKey::ArrowUp),
        "DOWN" => named(PhysicalKey::ArrowDown, NamedKey::ArrowDown),
        "LEFT" => named(PhysicalKey::ArrowLeft, NamedKey::ArrowLeft),
        "RIGHT" => named(PhysicalKey::ArrowRight, NamedKey::ArrowRight),
        "SPACE" => Some(char_to_key_descriptor(' ')),
        "ALT" => Some(KeyDescriptor {
            physical_key: PhysicalKey::AltLeft,
            logical_key: LogicalKey::Named(NamedKey::Alt),
            key_location: KeyLocation::Left,
        }),
        "CTRL" => Some(KeyDescriptor {
            physical_key: PhysicalKey::ControlLeft,
            logical_key: LogicalKey::Named(NamedKey::Control),
            key_location: KeyLocation::Left,
        }),
        _ => {
            let mut chars = tag.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => {
                    Some(char_to_key_descriptor(c.to_ascii_lowercase()))
                }
                _ => None,
            }
        }
    }
}
