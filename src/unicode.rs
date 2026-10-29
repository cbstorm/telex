//! Vietnamese Unicode mapping: turns a base letter, its diacritic and a tone
//! into a single precomposed (NFC) character.

/// Diacritic attached to a letter (not the tone).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    #[default]
    None,
    /// â ê ô
    Circumflex,
    /// ă
    Breve,
    /// ơ ư
    Horn,
    /// đ
    Stroke,
}

/// Syllable tone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    None,
    /// sắc (s)
    Acute,
    /// huyền (f)
    Grave,
    /// hỏi (r)
    Hook,
    /// ngã (x)
    Tilde,
    /// nặng (j)
    Dot,
}

impl Tone {
    /// Telex key for a tone: s f r x j.
    pub fn from_key(key: char) -> Option<Tone> {
        match key.to_ascii_lowercase() {
            's' => Some(Tone::Acute),
            'f' => Some(Tone::Grave),
            'r' => Some(Tone::Hook),
            'x' => Some(Tone::Tilde),
            'j' => Some(Tone::Dot),
            _ => None,
        }
    }

    pub fn key(self) -> Option<char> {
        match self {
            Tone::None => None,
            Tone::Acute => Some('s'),
            Tone::Grave => Some('f'),
            Tone::Hook => Some('r'),
            Tone::Tilde => Some('x'),
            Tone::Dot => Some('j'),
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// One letter of the word being composed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Letter {
    /// Lowercase ASCII letter.
    pub base: char,
    pub mark: Mark,
    pub upper: bool,
}

impl Letter {
    /// A letter exactly as typed, with no diacritic.
    pub fn plain(key: char) -> Letter {
        Letter {
            base: key.to_ascii_lowercase(),
            mark: Mark::None,
            upper: key.is_ascii_uppercase(),
        }
    }

    pub fn with_mark(key: char, mark: Mark) -> Letter {
        Letter {
            mark,
            ..Letter::plain(key)
        }
    }

    pub fn to_char(self, tone: Tone) -> char {
        compose(self.base, self.mark, tone, self.upper)
    }

    /// Telex keys that produce this letter (without tone), e.g. `ô` → "oo".
    pub fn keys(self) -> String {
        let key = if self.upper {
            self.base.to_ascii_uppercase()
        } else {
            self.base
        };
        match self.mark {
            Mark::None => key.to_string(),
            Mark::Circumflex => format!("{key}{}", self.base),
            Mark::Breve | Mark::Horn => format!("{key}w"),
            Mark::Stroke => format!("{key}d"),
        }
    }
}

// Each row: no tone, sắc, huyền, hỏi, ngã, nặng.
const VOWEL_ROWS: [(char, Mark, &str); 12] = [
    ('a', Mark::None, "aáàảãạ"),
    ('a', Mark::Breve, "ăắằẳẵặ"),
    ('a', Mark::Circumflex, "âấầẩẫậ"),
    ('e', Mark::None, "eéèẻẽẹ"),
    ('e', Mark::Circumflex, "êếềểễệ"),
    ('i', Mark::None, "iíìỉĩị"),
    ('o', Mark::None, "oóòỏõọ"),
    ('o', Mark::Circumflex, "ôốồổỗộ"),
    ('o', Mark::Horn, "ơớờởỡợ"),
    ('u', Mark::None, "uúùủũụ"),
    ('u', Mark::Horn, "ưứừửữự"),
    ('y', Mark::None, "yýỳỷỹỵ"),
];

/// Combine a lowercase ASCII base letter with a diacritic and tone.
/// Combinations that do not exist in Vietnamese fall back to the base letter.
pub fn compose(base: char, mark: Mark, tone: Tone, upper: bool) -> char {
    let lower = if base == 'd' && mark == Mark::Stroke {
        'đ'
    } else {
        VOWEL_ROWS
            .iter()
            .find(|(b, m, _)| *b == base && *m == mark)
            .and_then(|(_, _, row)| row.chars().nth(tone.index()))
            .unwrap_or(base)
    };
    if upper {
        lower.to_uppercase().next().unwrap_or(lower)
    } else {
        lower
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_vowels_and_tones() {
        assert_eq!(compose('a', Mark::None, Tone::Acute, false), 'á');
        assert_eq!(compose('a', Mark::Breve, Tone::Dot, false), 'ặ');
        assert_eq!(compose('e', Mark::Circumflex, Tone::Dot, true), 'Ệ');
        assert_eq!(compose('o', Mark::Horn, Tone::Grave, false), 'ờ');
        assert_eq!(compose('u', Mark::Horn, Tone::Tilde, true), 'Ữ');
        assert_eq!(compose('d', Mark::Stroke, Tone::None, true), 'Đ');
        assert_eq!(compose('b', Mark::None, Tone::Acute, false), 'b');
    }

    #[test]
    fn letter_keys_round_trip() {
        assert_eq!(Letter::with_mark('O', Mark::Circumflex).keys(), "Oo");
        assert_eq!(Letter::with_mark('u', Mark::Horn).keys(), "uw");
        assert_eq!(Letter::with_mark('D', Mark::Stroke).keys(), "Dd");
    }
}
