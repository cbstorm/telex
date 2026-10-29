//! IBus front-end for the Telex engine.
//!
//! ibus-daemon starts this program (see `data/telex.xml`) with `--ibus`.
//! It connects to the IBus bus over D-Bus, owns the component name and
//! serves `org.freedesktop.IBus.Factory`; the daemon then asks the factory to
//! create an engine object per input context. Each engine shows the word
//! being composed as pre-edit text and commits it when a key that is not a
//! letter (space, punctuation, Enter, arrows...) is pressed.

use std::collections::HashMap;
use std::error::Error;
use std::process::Command;

use telex::TelexEngine;
use zbus::object_server::{ObjectServer, SignalEmitter};
use zbus::zvariant::{OwnedObjectPath, Value};
use zbus::{blocking, fdo, interface};

const BUS_NAME: &str = "org.freedesktop.IBus.Telex";
const ENGINE_NAME: &str = "telex";
const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";

// Modifier state bits (IBusModifierType).
const SHIFT_MASK: u32 = 1 << 0;
const LOCK_MASK: u32 = 1 << 1;
const RELEASE_MASK: u32 = 1 << 30;
const IGNORED_MASK: u32 = SHIFT_MASK | LOCK_MASK | (1 << 4) /* Num Lock */;

const KEY_BACKSPACE: u32 = 0xff08;

// IBusEnginePreeditFocusMode: on focus out or reset, ibus-daemon commits the
// pre-edit text itself, so the word being typed is never lost.
const PREEDIT_COMMIT: u32 = 1;

// IBusInputPurpose values for which no conversion must happen.
const PURPOSE_PASSWORD: u32 = 8;
const PURPOSE_PIN: u32 = 9;

// IBusAttrType / IBusAttrUnderline.
const ATTR_TYPE_UNDERLINE: u32 = 1;
const ATTR_UNDERLINE_SINGLE: u32 = 1;

/// Modifier keys pressed on their own must not end the current word.
fn is_modifier_key(keyval: u32) -> bool {
    matches!(keyval, 0xffe1..=0xffee | 0xfe01..=0xfe0f)
}

type Attachments = HashMap<String, Value<'static>>;

/// Serialize text as an IBusText: (s name, a{sv} attachments, s text, v attrs).
fn ibus_text(text: &str, underline: bool) -> Value<'static> {
    let mut attrs: Vec<Value<'static>> = Vec::new();
    if underline {
        let len = text.chars().count() as u32;
        attrs.push(Value::new((
            "IBusAttribute",
            Attachments::new(),
            ATTR_TYPE_UNDERLINE,
            ATTR_UNDERLINE_SINGLE,
            0u32,
            len,
        )));
    }
    let attr_list = Value::new(("IBusAttrList", Attachments::new(), attrs));
    Value::new(("IBusText", Attachments::new(), text.to_string(), attr_list))
}

#[derive(Default)]
struct Factory {
    engines_created: u32,
}

#[interface(name = "org.freedesktop.IBus.Factory")]
impl Factory {
    async fn create_engine(
        &mut self,
        name: &str,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<OwnedObjectPath> {
        if name != ENGINE_NAME {
            return Err(fdo::Error::InvalidArgs(format!("unknown engine: {name}")));
        }
        self.engines_created += 1;
        let path = format!(
            "/org/freedesktop/IBus/Engine/Telex/{}",
            self.engines_created
        );
        server.at(path.as_str(), Engine::default()).await?;
        server.at(path.as_str(), Service).await?;
        OwnedObjectPath::try_from(path).map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

/// `org.freedesktop.IBus.Service` lets the daemon destroy an engine object.
struct Service;

#[interface(name = "org.freedesktop.IBus.Service")]
impl Service {
    async fn destroy(
        &self,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let path = emitter.path().to_owned();
        server.remove::<Engine, _>(&path).await?;
        server.remove::<Service, _>(&path).await?;
        Ok(())
    }
}

#[derive(Default)]
struct Engine {
    telex: TelexEngine,
    /// The focused field is a password or PIN entry: pass keys through.
    bypass: bool,
    content_type: (u32, u32),
}

impl Engine {
    async fn update_preedit(&self, emitter: &SignalEmitter<'_>) -> zbus::Result<()> {
        let text = self.telex.composition();
        let cursor = text.chars().count() as u32;
        let visible = !text.is_empty();
        Self::update_preedit_text(
            emitter,
            &ibus_text(text, true),
            cursor,
            visible,
            PREEDIT_COMMIT,
        )
        .await
    }

    /// Commit the word being composed, if any.
    async fn flush(&mut self, emitter: &SignalEmitter<'_>) -> zbus::Result<()> {
        if !self.telex.is_composing() {
            return Ok(());
        }
        let text = self.telex.commit();
        Self::update_preedit_text(emitter, &ibus_text("", false), 0, false, PREEDIT_COMMIT).await?;
        Self::commit_text(emitter, &ibus_text(&text, false)).await
    }

    async fn handle_key(
        &mut self,
        keyval: u32,
        state: u32,
        emitter: &SignalEmitter<'_>,
    ) -> zbus::Result<bool> {
        if state & RELEASE_MASK != 0 || self.bypass {
            return Ok(false);
        }
        if state & !IGNORED_MASK != 0 {
            // Ctrl/Alt/Super shortcuts: finish the word, let the app have the key.
            self.flush(emitter).await?;
            return Ok(false);
        }
        if keyval == KEY_BACKSPACE {
            if !self.telex.backspace() {
                return Ok(false);
            }
            self.update_preedit(emitter).await?;
            return Ok(true);
        }
        if let Some(key) = char::from_u32(keyval).filter(char::is_ascii_alphabetic) {
            self.telex.process_key(key);
            self.update_preedit(emitter).await?;
            return Ok(true);
        }
        if !is_modifier_key(keyval) {
            self.flush(emitter).await?;
        }
        Ok(false)
    }
}

#[interface(name = "org.freedesktop.IBus.Engine")]
impl Engine {
    async fn process_key_event(
        &mut self,
        keyval: u32,
        _keycode: u32,
        state: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<bool> {
        self.handle_key(keyval, state, &emitter)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    // ibus-daemon has already committed the pre-edit text (PREEDIT_COMMIT),
    // so the engine only drops its state.
    fn focus_out(&mut self) {
        self.telex.reset();
    }

    fn focus_out_id(&mut self, _object_path: &str) {
        self.telex.reset();
    }

    fn reset(&mut self) {
        self.telex.reset();
    }

    fn disable(&mut self) {
        self.telex.reset();
    }

    fn focus_in(&mut self) {}

    fn focus_in_id(&mut self, _object_path: &str, _client: &str) {}

    fn enable(&mut self) {}

    fn set_cursor_location(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}

    fn set_capabilities(&self, _caps: u32) {}

    fn set_surrounding_text(&self, _text: Value<'_>, _cursor_pos: u32, _anchor_pos: u32) {}

    fn process_hand_writing_event(&self, _coordinates: Vec<f64>) {}

    fn cancel_hand_writing(&self, _n_strokes: u32) {}

    fn property_activate(&self, _name: &str, _state: u32) {}

    fn property_show(&self, _name: &str) {}

    fn property_hide(&self, _name: &str) {}

    fn candidate_clicked(&self, _index: u32, _button: u32, _state: u32) {}

    fn page_up(&self) {}

    fn page_down(&self) {}

    fn cursor_up(&self) {}

    fn cursor_down(&self) {}

    fn panel_extension_received(&self, _event: Value<'_>) {}

    fn panel_extension_register_keys(&self, _data: Value<'_>) {}

    #[zbus(property)]
    fn content_type(&self) -> (u32, u32) {
        self.content_type
    }

    #[zbus(property)]
    fn set_content_type(&mut self, value: (u32, u32)) {
        self.content_type = value;
        self.bypass = matches!(value.0, PURPOSE_PASSWORD | PURPOSE_PIN);
        if self.bypass {
            self.telex.reset();
        }
    }

    // Introspection says "(b)" but ibus-daemon reads these as plain booleans.
    #[zbus(property)]
    fn focus_id(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn active_surrounding_text(&self) -> bool {
        false
    }

    #[zbus(signal)]
    async fn commit_text(emitter: &SignalEmitter<'_>, text: &Value<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_preedit_text(
        emitter: &SignalEmitter<'_>,
        text: &Value<'_>,
        cursor_pos: u32,
        visible: bool,
        mode: u32,
    ) -> zbus::Result<()>;
}

/// Find the IBus bus address the same way libibus does: `IBUS_ADDRESS`, or
/// the address file `~/.config/ibus/bus/<machine-id>-<host>-<display>`,
/// falling back to `ibus address`.
fn ibus_address() -> Result<String, Box<dyn Error>> {
    if let Ok(address) = std::env::var("IBUS_ADDRESS")
        && !address.is_empty()
    {
        return Ok(address);
    }
    if let Some(address) = address_from_file() {
        return Ok(address);
    }
    let output = Command::new("ibus").arg("address").output()?;
    let address = String::from_utf8(output.stdout)?.trim().to_string();
    if !output.status.success() || address.is_empty() || address == "(null)" {
        return Err("cannot find the IBus address; is ibus-daemon running?".into());
    }
    Ok(address)
}

fn address_from_file() -> Option<String> {
    let path = match std::env::var("IBUS_ADDRESS_FILE") {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => {
            let machine_id = ["/etc/machine-id", "/var/lib/dbus/machine-id"]
                .iter()
                .find_map(|p| std::fs::read_to_string(p).ok())?;
            let (host, display) = match std::env::var("WAYLAND_DISPLAY") {
                Ok(wayland) => ("unix".to_string(), wayland),
                Err(_) => {
                    let display = std::env::var("DISPLAY").ok()?;
                    let (host, rest) = display.split_once(':')?;
                    let number = rest.split('.').next()?;
                    let host = if host.is_empty() { "unix" } else { host };
                    (host.to_string(), number.to_string())
                }
            };
            let config = std::env::var("XDG_CONFIG_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|_| {
                    std::env::var("HOME").map(|h| std::path::Path::new(&h).join(".config"))
                })
                .ok()?;
            config
                .join("ibus/bus")
                .join(format!("{}-{host}-{display}", machine_id.trim()))
        }
    };
    let contents = std::fs::read_to_string(path).ok()?;
    contents
        .lines()
        .find_map(|line| line.strip_prefix("IBUS_ADDRESS="))
        .map(str::to_string)
}

fn main() -> Result<(), Box<dyn Error>> {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            // Passed by ibus-daemon when it starts the component.
            "--ibus" | "-i" => {}
            "-h" | "--help" => {
                println!("usage: ibus-engine-telex [--ibus]");
                println!("Vietnamese Telex engine for IBus; normally started by ibus-daemon.");
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    let address = ibus_address()?;
    let connection = blocking::connection::Builder::address(address.as_str())?
        .serve_at(FACTORY_PATH, Factory::default())?
        .name(BUS_NAME)?
        .build()?;

    // Requests are served on zbus's own executor thread. Keep the process
    // alive until the bus connection goes away (ibus-daemon exit or restart).
    for message in blocking::MessageIterator::from(&connection) {
        if message.is_err() {
            break;
        }
    }
    Ok(())
}
