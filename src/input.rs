//! Soft keyboard helpers that don't depend on Android or Ruffle types, so they can be unit
//! tested on any machine (`rustc --edition 2021 --test src/input.rs && ./input`).

/// What changed in the soft keyboard's text since the last update.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TextEdit {
    /// Number of characters deleted at the end.
    pub backspaces: usize,
    /// Text appended after the deletions.
    pub inserted: String,
}

/// Turns the successive text states the soft keyboard (IME) reports into key presses.
///
/// The IME edits its own copy of the text, and with GameActivity we only ever get to see the
/// complete new state. Flash text fields don't expose their contents to us, so instead of
/// mirroring them, the IME starts out empty whenever the keyboard is opened, and every change
/// is replayed as Backspaces plus typed characters. That also handles autocorrection and word
/// suggestions, which replace already typed text.
#[derive(Debug, Default)]
pub struct ImeTextTracker {
    last: String,
}

impl ImeTextTracker {
    pub fn update(&mut self, text: &str) -> TextEdit {
        let common = self
            .last
            .chars()
            .zip(text.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let edit = TextEdit {
            backspaces: self.last.chars().count() - common,
            inserted: text.chars().skip(common).collect(),
        };
        if edit.backspaces > 0 || !edit.inserted.is_empty() {
            self.last = text.to_owned();
        }
        edit
    }

    /// Forgets the text, for when the IME's text is cleared as well.
    pub fn reset(&mut self) {
        self.last.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(backspaces: usize, inserted: &str) -> TextEdit {
        TextEdit {
            backspaces,
            inserted: inserted.to_owned(),
        }
    }

    #[test]
    fn typing_and_deleting() {
        let mut ime = ImeTextTracker::default();
        assert_eq!(ime.update("h"), edit(0, "h"));
        assert_eq!(ime.update("he"), edit(0, "e"));
        assert_eq!(ime.update("hel"), edit(0, "l"));
        assert_eq!(ime.update("he"), edit(1, ""));
        assert_eq!(ime.update("he"), edit(0, ""));
        assert_eq!(ime.update(""), edit(2, ""));
    }

    #[test]
    fn autocorrect_replaces_word() {
        let mut ime = ImeTextTracker::default();
        ime.update("teh");
        assert_eq!(ime.update("the "), edit(2, "he "));
    }

    #[test]
    fn suggestion_and_multibyte() {
        let mut ime = ImeTextTracker::default();
        ime.update("grü");
        assert_eq!(ime.update("grüße "), edit(0, "ße "));
        assert_eq!(ime.update("grüß"), edit(2, ""));
        assert_eq!(ime.update("grüß😀"), edit(0, "😀"));
    }

    #[test]
    fn reset_forgets_text() {
        let mut ime = ImeTextTracker::default();
        ime.update("abc");
        ime.reset();
        assert_eq!(ime.update(""), edit(0, ""));
        assert_eq!(ime.update("x"), edit(0, "x"));
    }
}
