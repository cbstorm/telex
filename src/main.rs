//! Command-line Telex converter.
//!
//!     telex "Tooi yeeu Vieetj Nam"    # converts the arguments
//!     echo "xin chaof" | telex        # converts stdin line by line
//!     telex --modern "hoaf"           # modern tone placement: hoà

use std::io::{self, BufRead, IsTerminal, Write};

use telex::TelexEngine;

fn convert(text: &str, modern: bool) -> String {
    let mut engine = TelexEngine::new().with_modern_tone(modern);
    for key in text.chars() {
        engine.process_key(key);
    }
    engine.commit()
}

fn main() -> io::Result<()> {
    let mut modern = false;
    let mut words = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--modern" => modern = true,
            "-h" | "--help" => {
                println!("usage: telex [--modern] [TEXT...]");
                println!("Converts Telex input to Vietnamese. Reads stdin when no TEXT is given.");
                return Ok(());
            }
            _ => words.push(arg),
        }
    }

    if !words.is_empty() {
        println!("{}", convert(&words.join(" "), modern));
        return Ok(());
    }

    let interactive = io::stdin().is_terminal();
    let mut stdout = io::stdout();
    if interactive {
        eprintln!("Type Telex text and press Enter (Ctrl+D to quit).");
    }
    for line in io::stdin().lock().lines() {
        writeln!(stdout, "{}", convert(&line?, modern))?;
    }
    Ok(())
}
