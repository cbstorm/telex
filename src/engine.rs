//! The Telex state machine.
//!
//! The engine composes one word at a time. Each word is kept as a list of
//! letters (base letter + diacritic + case) and a single tone for the
//! syllable; the tone is placed on the right vowel only when the word is
//! rendered, so it moves as the word grows ("hoaf" → "hòa", "hoafng" →
//! "hoàng").
//!
//! Telex keys:
//!
//! | keys            | result                      |
//! |-----------------|-----------------------------|
//! | aa ee oo        | â ê ô                       |
//! | aw ow uw, uow   | ă ơ ư, ươ                   |
//! | w (no vowel)    | ư                           |
//! | dd              | đ                           |
//! | s f r x j       | sắc huyền hỏi ngã nặng      |
//! | z               | remove tone                 |
//!
//! Typing a transform key twice undoes it and keeps the key literally
//! ("aaa" → "aa", "ass" → "as", "ddd" → "dd"). When the letters stop
//! forming a possible Vietnamese syllable ("student", "class", "where"), the
//! word falls back to exactly what was typed and is no longer transformed.

use crate::syllable;
use crate::unicode::{Letter, Mark, Tone};

#[derive(Clone, Debug, Default)]
pub struct TelexEngine {
    letters: Vec<Letter>,
    tone: Tone,
    /// Keys typed for the current word, used when falling back to literal text.
    raw: String,
    /// Stop interpreting Telex keys until the word ends.
    literal: bool,
    /// Rendered current word.
    composition: String,
    /// Finished text not yet taken by the caller.
    committed: String,
    modern_tone: bool,
}

impl TelexEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use the modern tone placement for "oa", "oe", "uy" (hoà, khoẻ, thuỷ)
    /// instead of the traditional one (hòa, khỏe, thủy).
    pub fn with_modern_tone(mut self, modern: bool) -> Self {
        self.modern_tone = modern;
        self
    }

    /// Feed one typed character and return the current composition.
    ///
    /// ASCII letters are composed into the current word. Any other character
    /// ends the word: the word and the character move to the committed text
    /// (see [`TelexEngine::commit`]) and the composition becomes empty.
    pub fn process_key(&mut self, key: char) -> &str {
        if !key.is_ascii_alphabetic() {
            self.finish_word();
            self.committed.push(key);
            return &self.composition;
        }
        self.raw.push(key);
        if self.literal {
            self.letters.push(Letter::plain(key));
        } else {
            if !self.apply_telex(key) {
                self.letters.push(Letter::plain(key));
            }
            if !self.literal && !syllable::is_valid_prefix(&self.letters) {
                self.fall_back_to_raw();
            }
        }
        self.render();
        &self.composition
    }

    /// Delete the last visible character of the composition. Returns `false`
    /// when there is nothing being composed, so the caller should let the
    /// application handle the key.
    ///
    /// The tone stays on the syllable and moves if needed ("hoàng" → "hoàn"
    /// → "hòa"); it is dropped together with the vowel that carried it.
    pub fn backspace(&mut self) -> bool {
        if self.letters.is_empty() {
            return false;
        }
        let toned = self.tone_position();
        self.letters.pop();
        if toned == Some(self.letters.len()) {
            self.tone = Tone::None;
        }
        if self.letters.is_empty() {
            self.clear_word();
        } else {
            self.raw = self.letters.iter().map(|l| l.keys()).collect();
            self.raw.extend(self.tone.key());
        }
        self.render();
        true
    }

    /// The word currently being composed (shown as pre-edit text).
    pub fn composition(&self) -> &str {
        &self.composition
    }

    pub fn is_composing(&self) -> bool {
        !self.letters.is_empty()
    }

    /// Finish the current word and return all committed text.
    pub fn commit(&mut self) -> String {
        self.finish_word();
        std::mem::take(&mut self.committed)
    }

    /// Discard the current word and any committed text not yet taken.
    pub fn reset(&mut self) {
        self.clear_word();
        self.committed.clear();
    }

    /// Returns `true` when the key was used as a Telex command.
    fn apply_telex(&mut self, key: char) -> bool {
        match key.to_ascii_lowercase() {
            'd' => self.apply_stroke(key),
            'a' | 'e' | 'o' => self.apply_circumflex(key),
            'w' => self.apply_horn(key),
            'z' => self.remove_tone(),
            _ => match Tone::from_key(key) {
                Some(tone) => self.apply_tone(key, tone),
                None => false,
            },
        }
    }

    /// dd → đ (only as the first letter, where đ can occur).
    fn apply_stroke(&mut self, key: char) -> bool {
        match self.letters.as_mut_slice() {
            [first] if first.base == 'd' && first.mark == Mark::None => {
                first.mark = Mark::Stroke;
                true
            }
            [first] if first.mark == Mark::Stroke => {
                first.mark = Mark::None;
                self.undo_with(key);
                true
            }
            _ => false,
        }
    }

    /// aa ee oo → â ê ô; also turns ă/ơ into â/ô.
    fn apply_circumflex(&mut self, key: char) -> bool {
        let base = key.to_ascii_lowercase();
        let n = self.letters.len();
        let parts = syllable::split(&self.letters);
        // The doubled vowel must be the last letter typed and part of the
        // nucleus, so "photo" or "banana" are not transformed.
        if n == 0 || parts.nucleus.end != n || parts.nucleus.is_empty() {
            return false;
        }
        if self.letters[n - 1].base != base {
            return false;
        }
        match self.letters[n - 1].mark {
            Mark::Circumflex => {
                self.letters[n - 1].mark = Mark::None;
                self.undo_with(key);
            }
            Mark::Horn if n >= 2 && self.letters[n - 2].mark == Mark::Horn => {
                // ươ + o → uô
                self.letters[n - 1].mark = Mark::Circumflex;
                self.letters[n - 2].mark = Mark::None;
            }
            _ => self.letters[n - 1].mark = Mark::Circumflex,
        }
        true
    }

    /// w adds a breve or horn to the nucleus wherever it is in the word
    /// ("dduowcj" and "dduocwj" both give "được"), or types ư on its own.
    fn apply_horn(&mut self, key: char) -> bool {
        let parts = syllable::split(&self.letters);
        let nucleus = parts.nucleus.clone();
        if nucleus.is_empty() {
            if parts.coda.is_empty() {
                // "w" → ư, "nhw" → như
                let u = if key.is_ascii_uppercase() { 'U' } else { 'u' };
                self.letters.push(Letter::with_mark(u, Mark::Horn));
                return true;
            }
            return false;
        }
        let bases: String = self.letters[nucleus.clone()]
            .iter()
            .map(|l| l.base)
            .collect();
        let at = |offset: usize, mark: Mark| (nucleus.start + offset, mark);
        let targets: Vec<(usize, Mark)> = if let Some(i) = bases.find("uo") {
            vec![at(i, Mark::Horn), at(i + 1, Mark::Horn)]
        } else {
            match bases.as_str() {
                "a" => vec![at(0, Mark::Breve)],
                "oa" => vec![at(1, Mark::Breve)],
                "o" | "oi" => vec![at(0, Mark::Horn)],
                "u" | "ui" | "uu" | "ua" => vec![at(0, Mark::Horn)],
                _ => vec![],
            }
        };
        if targets.is_empty() {
            return false;
        }
        if targets
            .iter()
            .all(|&(i, mark)| self.letters[i].mark == mark)
        {
            if self.typed_lone_w(&targets) {
                // "ww" → "w": the ư came from a w on its own, give the w back.
                let i = targets[0].0;
                let w = self.raw.chars().rev().nth(1).unwrap_or(key);
                self.letters[i] = Letter::plain(w);
                self.literal = true;
                return true;
            }
            for (i, _) in targets {
                self.letters[i].mark = Mark::None;
            }
            self.undo_with(key);
        } else {
            for (i, mark) in targets {
                self.letters[i].mark = mark;
            }
        }
        true
    }

    /// Was the single horned target the ư produced by the previous key being
    /// a lone w (not "uw")? `raw` already ends with the current key.
    fn typed_lone_w(&self, targets: &[(usize, Mark)]) -> bool {
        let [(i, Mark::Horn)] = targets else {
            return false;
        };
        let mut previous = self.raw.chars().rev().skip(1);
        *i + 1 == self.letters.len()
            && previous
                .next()
                .is_some_and(|c| c.eq_ignore_ascii_case(&'w'))
            && !previous
                .next()
                .is_some_and(|c| c.eq_ignore_ascii_case(&'u'))
    }

    fn apply_tone(&mut self, key: char, tone: Tone) -> bool {
        if syllable::split(&self.letters).nucleus.is_empty()
            || !syllable::tone_allowed(&self.letters, tone)
        {
            return false;
        }
        if self.tone == tone {
            self.tone = Tone::None;
            self.undo_with(key);
        } else {
            self.tone = tone;
        }
        true
    }

    fn remove_tone(&mut self) -> bool {
        if self.tone == Tone::None {
            return false;
        }
        self.tone = Tone::None;
        true
    }

    /// The transform was cancelled by repeating its key: type the key itself
    /// and leave the rest of the word alone.
    fn undo_with(&mut self, key: char) {
        self.letters.push(Letter::plain(key));
        self.literal = true;
    }

    fn fall_back_to_raw(&mut self) {
        self.letters = self.raw.chars().map(Letter::plain).collect();
        self.tone = Tone::None;
        self.literal = true;
    }

    fn tone_position(&self) -> Option<usize> {
        if self.tone == Tone::None {
            return None;
        }
        syllable::tone_position(&self.letters, self.modern_tone)
    }

    fn render(&mut self) {
        let toned = self.tone_position();
        self.composition = self
            .letters
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let tone = if Some(i) == toned {
                    self.tone
                } else {
                    Tone::None
                };
                l.to_char(tone)
            })
            .collect();
    }

    fn finish_word(&mut self) {
        let word = std::mem::take(&mut self.composition);
        self.committed.push_str(&word);
        self.clear_word();
    }

    fn clear_word(&mut self) {
        self.letters.clear();
        self.tone = Tone::None;
        self.raw.clear();
        self.literal = false;
        self.composition.clear();
    }
}

/// Convert a whole string as if it were typed key by key.
pub fn convert(input: &str) -> String {
    let mut engine = TelexEngine::new();
    for key in input.chars() {
        engine.process_key(key);
    }
    engine.commit()
}
