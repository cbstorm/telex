//! Regression tests for the Telex engine.

use crate::{TelexEngine, convert};

/// Type `input` into a fresh engine; `{BS}` presses Backspace.
fn type_keys(input: &str) -> String {
    let mut engine = TelexEngine::new();
    for (i, chunk) in input.split("{BS}").enumerate() {
        if i > 0 {
            engine.backspace();
        }
        for key in chunk.chars() {
            engine.process_key(key);
        }
    }
    engine.commit()
}

#[test]
fn data_file_cases() {
    let data = include_str!("../data/test-cases.txt");
    let mut failures = Vec::new();
    let mut count = 0;
    for line in data.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (input, expected) = line
            .split_once('\t')
            .unwrap_or_else(|| panic!("missing tab in test case: {line:?}"));
        count += 1;
        let actual = type_keys(input);
        if actual != expected {
            failures.push(format!("{input:?}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(count > 0);
    assert!(
        failures.is_empty(),
        "{} failing cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn required_examples() {
    assert_eq!(convert("tieengs"), "tiếng");
    assert_eq!(convert("Vieetj"), "Việt");
    assert_eq!(convert("ddawng"), "đăng");
    assert_eq!(convert("xin chaof"), "xin chào");
    assert_eq!(convert("Tooi yeeu Vieetj Nam"), "Tôi yêu Việt Nam");
}

#[test]
fn composition_is_separate_from_committed_text() {
    let mut engine = TelexEngine::new();
    assert_eq!(engine.process_key('t'), "t");
    assert_eq!(engine.process_key('o'), "to");
    assert_eq!(engine.process_key('o'), "tô");
    assert_eq!(engine.process_key('i'), "tôi");
    assert!(engine.is_composing());
    assert_eq!(engine.process_key(' '), "");
    assert!(!engine.is_composing());
    assert_eq!(engine.process_key('a'), "a");
    assert_eq!(engine.process_key('f'), "à");
    assert_eq!(engine.commit(), "tôi à");
    assert_eq!(engine.commit(), "");
}

#[test]
fn backspace_on_empty_composition_is_not_consumed() {
    let mut engine = TelexEngine::new();
    assert!(!engine.backspace());
    engine.process_key('a');
    assert!(engine.backspace());
    assert!(!engine.backspace());
}

#[test]
fn literal_word_restarts_after_separator() {
    assert_eq!(convert("aaa aa"), "aa â");
    assert_eq!(convert("class tieengs"), "class tiếng");
}

#[test]
fn reset_discards_everything() {
    let mut engine = TelexEngine::new();
    for key in "chaof ban".chars() {
        engine.process_key(key);
    }
    engine.reset();
    assert_eq!(engine.composition(), "");
    assert_eq!(engine.commit(), "");
}

#[test]
fn modern_tone_placement() {
    let mut engine = TelexEngine::new().with_modern_tone(true);
    for key in "hoaf thuyr".chars() {
        engine.process_key(key);
    }
    assert_eq!(engine.commit(), "hoà thuỷ");
}
