#!/usr/bin/env python3
"""End-to-end check of ibus-engine-telex through a real ibus-daemon.

Acts as an application: creates an input context, selects the telex
engine, sends key events and rebuilds the text the application would show
(committed text, plus keys the engine did not consume, plus pre-edit text).
Run it through run.sh, which starts an isolated ibus-daemon.
"""

import sys
import time

import gi

gi.require_version("IBus", "1.0")
from gi.repository import GLib, IBus  # noqa: E402

BACKSPACE = "\b"


def pump(seconds=0.05):
    context = GLib.MainContext.default()
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        while context.pending():
            context.iteration(False)
        time.sleep(0.005)


class App:
    def __init__(self, bus, purpose=IBus.InputPurpose.FREE_FORM):
        self.text = ""
        self.preedit = ""
        self.ctx = bus.create_input_context("telex-e2e")
        self.ctx.set_capabilities(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.FOCUS)
        self.ctx.connect("commit-text", self.on_commit)
        self.ctx.connect("update-preedit-text", self.on_preedit)
        self.ctx.connect("hide-preedit-text", lambda *_: self.set_preedit(""))
        self.ctx.focus_in()
        self.ctx.set_content_type(purpose, 0)
        bus.set_global_engine("telex")
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            pump(0.1)
            engine = self.ctx.get_engine()
            if engine is not None and engine.get_name() == "telex":
                return
        raise RuntimeError("engine telex was not activated")

    def on_commit(self, _ctx, text):
        self.text += text.get_text()

    def on_preedit(self, _ctx, text, _cursor, visible):
        self.set_preedit(text.get_text() if visible else "")

    def set_preedit(self, text):
        self.preedit = text

    def press(self, keyval, state=0):
        handled = self.ctx.process_key_event(keyval, 0, state)
        self.ctx.process_key_event(keyval, 0, state | IBus.ModifierType.RELEASE_MASK)
        pump()
        return handled

    def type(self, keys):
        for key in keys:
            if key == BACKSPACE:
                if not self.press(IBus.KEY_BackSpace):
                    self.text = self.text[:-1]
                continue
            state = IBus.ModifierType.SHIFT_MASK if key.isupper() else 0
            if not self.press(ord(key), state):
                # Not consumed: the application inserts the character.
                self.text += key

    def shown(self):
        return self.text + self.preedit


def main():
    IBus.init()
    bus = IBus.Bus()
    if not bus.is_connected():
        sys.exit("cannot connect to ibus-daemon")

    failures = 0

    def check(name, actual, expected):
        nonlocal failures
        ok = actual == expected
        failures += not ok
        print(f"{'ok  ' if ok else 'FAIL'} {name}: {actual!r}" + ("" if ok else f" (expected {expected!r})"))

    cases = [
        ("Tooi yeeu Vieetj Nam ", "Tôi yêu Việt Nam "),
        ("xin chaof, tieengs Vieetj! ", "xin chào, tiếng Việt! "),
        ("dduowcj nguoiwf ", "được người "),
        ("tieengs\b\b ", "tiế "),
        ("ab\b\b\bX", "X"),
        ("student class ", "student class "),
        ("aaa ass ", "aa as "),
    ]
    for keys, expected in cases:
        app = App(bus)
        app.type(keys)
        check(repr(keys), app.shown(), expected)

    # Pre-edit while composing, then commit on Enter.
    app = App(bus)
    app.type("hoafng")
    check("preedit", (app.text, app.preedit), ("", "hoàng"))
    app.press(IBus.KEY_Return)
    check("enter commits", (app.text, app.preedit), ("hoàng", ""))

    # Losing focus mid-word keeps the word (ibus-daemon commits the pre-edit).
    app = App(bus)
    app.type("Vieetj")
    app.ctx.focus_out()
    pump(0.2)
    check("focus out commits", (app.text, app.preedit), ("Việt", ""))

    # Ctrl shortcuts finish the word and are not consumed.
    app = App(bus)
    app.type("chaof")
    handled = app.press(ord("c"), IBus.ModifierType.CONTROL_MASK)
    check("ctrl+c", (app.text, app.preedit, handled), ("chào", "", False))

    # Password fields are never converted.
    app = App(bus, IBus.InputPurpose.PASSWORD)
    app.type("aas")
    check("password field", app.shown(), "aas")

    print("all passed" if failures == 0 else f"{failures} failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
