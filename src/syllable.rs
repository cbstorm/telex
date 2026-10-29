//! Vietnamese syllable structure: splitting a word into initial consonant,
//! vowel nucleus and final consonant, checking that what has been typed can
//! still become a real syllable, and deciding which vowel carries the tone.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::OnceLock;

use crate::unicode::{Letter, Mark, Tone};

const INITIALS: [&str; 27] = [
    "", "b", "c", "ch", "d", "g", "gh", "gi", "h", "k", "kh", "l", "m", "n", "ng", "ngh", "nh",
    "p", "ph", "qu", "r", "s", "t", "th", "tr", "v", "x",
];

/// Vowel nuclei by base letter (diacritics ignored) and whether a final
/// consonant may follow. Nuclei ending in a semivowel (i/y/o/u) are closed.
const NUCLEI: [(&str, bool); 38] = [
    ("a", true),
    ("e", true),
    ("i", true),
    ("o", true),
    ("u", true),
    ("y", false),
    ("ai", false),
    ("ao", false),
    ("au", false),
    ("ay", false),
    ("eo", false),
    ("eu", false),
    ("ia", false),
    ("ie", true),
    ("iu", false),
    ("oa", true),
    ("oe", true),
    ("oi", false),
    ("oo", true),
    ("ua", true),
    ("ue", true),
    ("ui", false),
    ("uo", true),
    ("uu", false),
    ("uy", true),
    ("ye", true),
    ("ieu", false),
    ("yeu", false),
    ("oai", false),
    ("oao", false),
    ("oay", false),
    ("oeo", false),
    ("uay", false),
    ("uoi", false),
    ("uou", false),
    ("uya", false),
    ("uye", true),
    ("uyu", false),
];

const FINALS: [&str; 8] = ["c", "ch", "m", "n", "ng", "nh", "p", "t"];

pub fn is_vowel(base: char) -> bool {
    matches!(base, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Index ranges of the three parts of a syllable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parts {
    pub initial: Range<usize>,
    pub nucleus: Range<usize>,
    pub coda: Range<usize>,
}

/// Split letters into initial consonant, vowel nucleus and final consonant.
/// The `u` of "qu" and the `i` of "gi" (before another vowel) belong to the
/// initial consonant.
pub fn split(letters: &[Letter]) -> Parts {
    let n = letters.len();
    let base = |i: usize| letters[i].base;
    let mut i = 0;
    while i < n && !is_vowel(base(i)) {
        i += 1;
    }
    if i == 1 && i < n {
        let qu = base(0) == 'q' && base(1) == 'u';
        let gi = base(0) == 'g' && base(1) == 'i' && i + 1 < n && is_vowel(base(2));
        if qu || gi {
            i += 1;
        }
    }
    let start = i;
    while i < n && is_vowel(base(i)) {
        i += 1;
    }
    Parts {
        initial: 0..start,
        nucleus: start..i,
        coda: i..n,
    }
}

fn valid_prefixes() -> &'static HashSet<String> {
    static PREFIXES: OnceLock<HashSet<String>> = OnceLock::new();
    PREFIXES.get_or_init(|| {
        let mut set = HashSet::new();
        for initial in INITIALS {
            for (nucleus, open) in NUCLEI {
                if !spelling_allowed(initial, nucleus) {
                    continue;
                }
                let finals: &[&str] = if open { &FINALS } else { &[] };
                for coda in std::iter::once("").chain(finals.iter().copied()) {
                    let word = format!("{initial}{nucleus}{coda}");
                    for end in 0..=word.len() {
                        set.insert(word[..end].to_string());
                    }
                }
            }
        }
        set
    })
}

/// Vietnamese spelling rules: k/gh/ngh only before e, i, y; c/ng never
/// before them; g never before e.
fn spelling_allowed(initial: &str, nucleus: &str) -> bool {
    let front = nucleus.starts_with(['e', 'i', 'y']);
    match initial {
        "k" | "gh" | "ngh" => front,
        "c" | "ng" => !front,
        "g" => !nucleus.starts_with('e'),
        "qu" => !nucleus.starts_with('u'),
        _ => true,
    }
}

/// Can these letters (diacritics ignored) still grow into a Vietnamese
/// syllable? Used to detect foreign words and stop transforming them.
pub fn is_valid_prefix(letters: &[Letter]) -> bool {
    let bases: String = letters.iter().map(|l| l.base).collect();
    valid_prefixes().contains(&bases)
}

/// The final consonant as lowercase ASCII.
pub fn coda(letters: &[Letter]) -> String {
    letters[split(letters).coda]
        .iter()
        .map(|l| l.base)
        .collect()
}

/// Syllables ending in c, ch, p or t only take the sắc or nặng tone.
pub fn tone_allowed(letters: &[Letter], tone: Tone) -> bool {
    match coda(letters).as_str() {
        "c" | "ch" | "p" | "t" => matches!(tone, Tone::Acute | Tone::Dot | Tone::None),
        _ => true,
    }
}

/// Index of the letter that carries the tone mark, or `None` when there is
/// no vowel. With `modern` set, "oa", "oe" and "uy" put the tone on the
/// second vowel (hoà, khoẻ, thuỷ); otherwise on the first (hòa, khỏe, thủy).
pub fn tone_position(letters: &[Letter], modern: bool) -> Option<usize> {
    let parts = split(letters);
    let nucleus = parts.nucleus.clone();
    if nucleus.is_empty() {
        // "gi" alone ("gì") is split as initial g + nucleus i, so this is
        // only reached for words without vowels.
        return None;
    }
    // A vowel with a diacritic always takes the tone; for ươ it is the ơ.
    if let Some(i) = nucleus
        .clone()
        .rev()
        .find(|&i| matches!(letters[i].mark, Mark::Circumflex | Mark::Breve | Mark::Horn))
    {
        return Some(i);
    }
    if !parts.coda.is_empty() {
        return Some(nucleus.end - 1);
    }
    match nucleus.len() {
        1 => Some(nucleus.start),
        2 => {
            let pair: String = letters[nucleus.clone()].iter().map(|l| l.base).collect();
            let second = modern && matches!(pair.as_str(), "oa" | "oe" | "uy");
            Some(nucleus.start + usize::from(second))
        }
        _ => Some(nucleus.start + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letters(s: &str) -> Vec<Letter> {
        s.chars().map(Letter::plain).collect()
    }

    #[test]
    fn splits_qu_and_gi() {
        let p = split(&letters("quang"));
        assert_eq!((p.initial, p.nucleus, p.coda), (0..2, 2..3, 3..5));
        let p = split(&letters("gia"));
        assert_eq!((p.initial, p.nucleus), (0..2, 2..3));
        let p = split(&letters("gin"));
        assert_eq!((p.initial, p.nucleus), (0..1, 1..2));
    }

    #[test]
    fn validates_prefixes() {
        for ok in ["", "ng", "ngh", "nghieng", "tie", "khuya", "quyen", "d"] {
            assert!(is_valid_prefix(&letters(ok)), "{ok}");
        }
        for bad in ["st", "cl", "wh", "pain", "bana", "ngi", "ce", "aa", "ge"] {
            assert!(!is_valid_prefix(&letters(bad)), "{bad}");
        }
    }

    #[test]
    fn places_tone() {
        assert_eq!(tone_position(&letters("hoa"), false), Some(1));
        assert_eq!(tone_position(&letters("hoa"), true), Some(2));
        assert_eq!(tone_position(&letters("hoang"), false), Some(2));
        assert_eq!(tone_position(&letters("ngoai"), false), Some(3));
        assert_eq!(tone_position(&letters("qua"), false), Some(2));
        assert_eq!(tone_position(&letters("gi"), false), Some(1));
        assert_eq!(tone_position(&letters("str"), false), None);
    }
}
