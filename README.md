# telex

A Vietnamese Telex input method written in Rust, with an IBus engine for
Ubuntu/GNOME. Type `tieengs Vieetj` and get `tiếng Việt` in any application.

## Layout

```
telex/
├── Cargo.toml
├── src/
│   ├── lib.rs              # library entry point: TelexEngine, convert()
│   ├── main.rs             # CLI converter
│   ├── engine.rs           # Telex state machine
│   ├── unicode.rs          # Vietnamese Unicode mapping
│   ├── syllable.rs         # syllable structure, validity, tone placement
│   ├── tests.rs            # regression tests
│   └── bin/ibus_engine.rs  # IBus engine (D-Bus, via zbus)
├── data/
│   ├── test-cases.txt      # input → expected output, run by `cargo test`
│   └── telex.xml.in        # IBus component descriptor template
├── tests/ibus/             # end-to-end test against a private ibus-daemon
└── install.sh
```

The conversion library has no dependencies and knows nothing about IBus;
the IBus front-end is a separate binary behind the default `ibus` feature.

## Telex rules

| Keys                | Result                                 |
| ------------------- | -------------------------------------- |
| `aa` `ee` `oo`      | â ê ô                                  |
| `aw` `ow` `uw`      | ă ơ ư (`uow` → ươ, `w` alone → ư)      |
| `dd`                | đ                                      |
| `s` `f` `r` `x` `j` | sắc, huyền, hỏi, ngã, nặng (á à ả ã ạ) |
| `z`                 | remove the tone                        |

- You can type tones and `w` anywhere in the word: `tieengs` and `tieesng`
  both give tiếng, `dduowcj` and `dduocwj` both give được. The tone goes on the correct vowel and moves as
  the word grows (`hoaf` → hòa, `hoafng` → hoàng).
- Repeating a key undoes it and types the letter: `aaa` → aa, `ass` → as,
  `ddd` → dd, `ww` → w.
- If the letters can no longer form a Vietnamese syllable, the word goes
  back to exactly what you typed and is left alone: `student`, `class`,
  `where`, `windows`, `email`.
- Backspace deletes the last visible character and keeps the tone
  (`tiếng` → `tiến` → `tiế` → `ti`).
- Tone placement is the traditional style by default (hòa, thủy, khỏe);
  `TelexEngine::with_modern_tone(true)` or `telex --modern` gives hoà,
  thuỷ, khoẻ.
- Uppercase works throughout: `VIEETJ` → VIỆT, `DDi` → Đi.

## Build and try it

```sh
sudo apt install -y build-essential pkg-config   # plus Rust via rustup
cargo build --release
cargo run --bin telex -- "Tooi yeeu Saif Gonf"   # Tôi yêu Sài Gòn
echo "xin chaof" | cargo run --bin telex          # xin chào
```

Library use:

```rust
let mut engine = telex::TelexEngine::new();
for key in "tieengs".chars() {
    engine.process_key(key);          // returns the composition: "tiếng"
}
assert_eq!(engine.commit(), "tiếng"); // finished text
```

`process_key` composes ASCII letters; any other character ends the word and
moves it to the committed text. `backspace()` returns `false` when nothing is
being composed, so the caller can pass the key on.

## Tests

```sh
cargo test               # unit tests + every case in data/test-cases.txt
tests/ibus/run.sh        # end-to-end through a real, isolated ibus-daemon
```

`tests/ibus/run.sh` starts its own `ibus-daemon` on a private address with
separate config and cache directories, so your desktop session is not
affected. It needs `python3-gi` and `gir1.2-ibus-1.0`.

## Install the IBus engine

```sh
./install.sh          # system-wide, asks for sudo, restarts IBus
./install.sh --user   # no sudo; log out and back in afterwards
```

Then add **Vietnamese (Telex)** in Settings → Keyboard → Input Sources and
switch with Super+Space. `./install.sh --uninstall` (plus `--user` if that
is how you installed it) removes it.

IBus only loads components from `/usr/share/ibus/component` unless
`IBUS_COMPONENT_PATH` is set. That is why the per-user install writes
`~/.config/environment.d/90-telex-ibus.conf` and needs a new login.

### How the IBus engine works

ibus-daemon starts `ibus-engine-telex --ibus`. The engine connects to the
IBus bus over D-Bus, owns `org.freedesktop.IBus.Telex`, and serves
`org.freedesktop.IBus.Factory`. Each input context gets its own engine
object. The word being typed is shown underlined as pre-edit text and
committed when you press space, punctuation, Enter, an arrow key or a
Ctrl/Alt shortcut. If focus moves away mid-word, ibus-daemon commits the
pre-edit, so nothing is lost. Password and PIN fields are passed through
untouched.

## Known limitations

- English words that are also valid Vietnamese syllables still convert, as
  in every Telex input method without a dictionary: `test` → tét, `is` → í.
  Type the tone key twice to keep the letter (`tesst` → test) or switch input
  source for English.
- `uow` always gives ươ, so `thuở` has to be typed as `thuowr` → thưở and
  corrected by hand.
- `aa`/`ee`/`oo` must be typed next to each other (`toon` → tôn, not
  `tono`). This keeps words like `photo` intact.
