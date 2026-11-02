"""Headless test client for the Ferry GNOME extension spike.

Drives every D-Bus method and signal, a real Ctrl+V into a GTK entry through the
extension's virtual keyboard, the picker shortcut, PresentPicker, the modifier wait
and re-registration after the extension is disabled and enabled.
"""
import os
import sys
import time

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gio, GLib, Gtk  # noqa: E402

BUS = "org.gnome.Shell"
PATH = "/org/gnome/Shell/Extensions/Ferry"
IFACE = "org.gnome.Shell.Extensions.Ferry"
UUID = "ferry@moorew.github.io"

conn = Gio.bus_get_sync(Gio.BusType.SESSION)
signals = []
results = []


def check(name, ok, detail=""):
    results.append((name, ok))
    print(f"{'PASS' if ok else 'FAIL'}  {name}{('  ' + detail) if detail else ''}", flush=True)


def pump(ms):
    end = time.monotonic() + ms / 1000
    ctx = GLib.MainContext.default()
    while time.monotonic() < end:
        while ctx.pending():
            ctx.iteration(False)
        time.sleep(0.005)


def call(method, args=None, sig=None, iface=IFACE, path=PATH):
    reply = conn.call_sync(BUS, path, iface, method, args,
                           GLib.VariantType(sig) if sig else None,
                           Gio.DBusCallFlags.NONE, 5000, None)
    return reply.unpack() if reply else None


def eval_js(code):
    ok, out = call("Eval", GLib.Variant("(s)", (code,)), "(bs)", "org.gnome.Shell", "/org/gnome/Shell")
    return ok, out


def on_signal(_conn, sender, path, iface, name, params):
    signals.append((time.monotonic(), name, params.unpack()))


conn.signal_subscribe(BUS, IFACE, None, PATH, None, Gio.DBusSignalFlags.NONE, on_signal)

# GNOME starts in the overview when no windows are open; leave it like a user would.
deadline = time.monotonic() + 40
state = ""
while time.monotonic() < deadline:
    state = eval_js("JSON.stringify([Main.layoutManager._startingUp, Main.overview.visible, Main.actionMode])")[1]
    if state == '"[false,false,1]"':
        break
    if state.startswith('"[true') and time.monotonic() > deadline - 30:
        # Headless shells sometimes never finish the startup animation.
        eval_js("Main.layoutManager._startupAnimationComplete(); 'ok'")
    if state.startswith('"[false,true'):
        eval_js("Main.overview.hide(); 'ok'")
    pump(250)
print(f"shell ready: {state}", flush=True)
pump(500)

# 1. Methods refuse callers that haven't registered.
try:
    call("GetMimeTypes")
    check("unregistered caller is refused", False)
except GLib.Error as e:
    check("unregistered caller is refused", "AccessDenied" in e.message, e.message.split(":")[-1].strip())

# 2. Register.
api = call("Register", GLib.Variant("(u)", (1,)), "(u)")
check("Register returns extension API 1", api == (1,), str(api))

# 3. SetContent text -> ClipboardChanged -> GetContent round trip.
signals.clear()
text = "hello from ferry ✓".encode()
call("SetContent", GLib.Variant("(say)", ("text/plain;charset=utf-8", text)))
pump(300)
changed = [s for s in signals if s[1] == "ClipboardChanged"]
check("ClipboardChanged after SetContent", bool(changed), str(changed[-1][2]) if changed else "no signal")
mimes = call("GetMimeTypes", None, "(as)")[0]
check("GetMimeTypes lists the text type", "text/plain;charset=utf-8" in mimes, str(mimes))
got = bytes(call("GetContent", GLib.Variant("(s)", ("text/plain;charset=utf-8",)), "(ay)")[0])
check("GetContent text round trip", got == text)

# 4. Binary content (PNG-sized blob) round trip, without PyGObject's slow 'ay' packing.
blob = bytes(range(256)) * 4096  # 1 MiB
packed = GLib.Variant.new_tuple(GLib.Variant.new_string("image/png"),
                                GLib.Variant.new_from_bytes(GLib.VariantType("ay"), GLib.Bytes.new(blob), True))
t = time.monotonic()
conn.call_sync(BUS, PATH, IFACE, "SetContent", packed, None, Gio.DBusCallFlags.NONE, 5000, None)
set_ms = (time.monotonic() - t) * 1000
pump(100)
t = time.monotonic()
reply = conn.call_sync(BUS, PATH, IFACE, "GetContent", GLib.Variant("(s)", ("image/png",)),
                       GLib.VariantType("(ay)"), Gio.DBusCallFlags.NONE, 5000, None)
got = reply.get_child_value(0).get_data_as_bytes().get_data()
get_ms = (time.monotonic() - t) * 1000
check("1 MiB binary round trip", got == blob, f"SetContent {set_ms:.0f} ms, GetContent {get_ms:.0f} ms")

# 5. SetShortcut writes the GSettings key.
call("SetShortcut", GLib.Variant("(sas)", ("picker", ["<Control><Alt>v"])))
call("SetShortcut", GLib.Variant("(sas)", ("paste-plain", ["<Control><Alt><Shift>v"])))
try:
    call("SetShortcut", GLib.Variant("(sas)", ("nope", [])))
    check("SetShortcut rejects unknown actions", False)
except GLib.Error:
    check("SetShortcut rejects unknown actions", True)
pump(300)
schema_dir = os.path.expanduser(f"~/.local/share/gnome-shell/extensions/{UUID}/schemas")
src = Gio.SettingsSchemaSource.new_from_directory(schema_dir, None, False)
settings = Gio.Settings.new_full(src.lookup("org.gnome.shell.extensions.ferry", False), None, None)
check("picker-shortcut stored", settings.get_strv("picker-shortcut") == ["<Control><Alt>v"],
      str(settings.get_strv("picker-shortcut")))

# 6. An "editor" window with a focused entry, then the picker shortcut.
editor = Gtk.Window(title="Editor")
entry = Gtk.Entry()
editor.add(entry)
editor.set_default_size(400, 100)
editor.show_all()
pump(800)
check("editor window is focused", editor.is_active())

PRESS = """
const seat = (global.stage.context?.get_backend() ?? imports.gi.Clutter.get_default_backend()).get_default_seat();
const C = imports.gi.Clutter;
globalThis._ferryTestKb ??= seat.create_virtual_device(C.InputDeviceType.KEYBOARD_DEVICE);
const kb = globalThis._ferryTestKb;
const t = imports.gi.GLib.get_monotonic_time();
const keys = [%s];
for (const k of keys) kb.notify_keyval(t, k, C.KeyState.%s);
'ok'
"""
CTRL_ALT_V = "C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v"

signals.clear()
eval_js(PRESS % (CTRL_ALT_V, "PRESSED"))
eval_js(PRESS % (CTRL_ALT_V, "RELEASED"))
pump(300)
shortcuts = [s for s in signals if s[1] == "ShortcutActivated"]
check("ShortcutActivated('picker') on Ctrl+Alt+V", bool(shortcuts) and shortcuts[-1][2][0] == "picker",
      str(shortcuts[-1][2]) if shortcuts else "no signal")

# 7. Show the picker after PresentPicker (the window appears just after the call).
picker = Gtk.Window(title="Ferry Picker")
picker.set_default_size(640, 420)
picker.add(Gtk.Label(label="picker"))
t = time.monotonic()
call("PresentPicker", GLib.Variant("(us)", (os.getpid(), "Ferry Picker")))
picker.show_all()
deadline = time.monotonic() + 1.5
while not picker.is_active() and time.monotonic() < deadline:
    pump(5)
check("PresentPicker focuses a window created after the call", picker.is_active(),
      f"{(time.monotonic() - t) * 1000:.0f} ms from PresentPicker")

# Hide and show again: GTK on Wayland makes a new toplevel each time.
picker.hide()
pump(300)
t = time.monotonic()
picker.show_all()
call("PresentPicker", GLib.Variant("(us)", (os.getpid(), "Ferry Picker")))
deadline = time.monotonic() + 1.5
while not picker.is_active() and time.monotonic() < deadline:
    pump(5)
check("PresentPicker focuses an already mapped window", picker.is_active(),
      f"{(time.monotonic() - t) * 1000:.0f} ms from show")

# 8. Choose: set clipboard, hide picker, Paste(ctrl-v, activate_previous=true).
call("SetContent", GLib.Variant("(say)", ("text/plain;charset=utf-8", b"pasted by ferry")))
picker.hide()
pump(30)
t = time.monotonic()
paste_done = []
conn.call(BUS, PATH, IFACE, "Paste", GLib.Variant("(sb)", ("ctrl-v", True)), None,
          Gio.DBusCallFlags.NONE, 5000, None, lambda c, r: paste_done.append(time.monotonic()))
while not paste_done and time.monotonic() - t < 2:
    pump(5)
pump(300)
check("Paste activates the previous window", editor.is_active())
check("Paste delivers Ctrl+V into the previous window", entry.get_text() == "pasted by ferry",
      repr(entry.get_text()))

# 9. Modifier wait: Paste fails cleanly while Shift is held, succeeds once released.
entry.set_text("")
eval_js(PRESS % ("C.KEY_Shift_L", "PRESSED"))
pump(50)
t = time.monotonic()
try:
    call("Paste", GLib.Variant("(sb)", ("ctrl-v", False)))
    check("Paste waits, then refuses while Shift is held", False)
except GLib.Error as e:
    ms = (time.monotonic() - t) * 1000
    check("Paste waits, then refuses while Shift is held", "ModifiersHeld" in e.message, f"after {ms:.0f} ms")
eval_js(PRESS % ("C.KEY_Shift_L", "PRESSED"))
GLib.timeout_add(150, lambda: eval_js(PRESS % ("C.KEY_Shift_L", "RELEASED")) and False)
t = time.monotonic()
call_done = []
conn.call(BUS, PATH, IFACE, "Paste", GLib.Variant("(sb)", ("ctrl-v", False)), None,
          Gio.DBusCallFlags.NONE, 5000, None, lambda c, r: call_done.append(time.monotonic()))
while not call_done and time.monotonic() - t < 2:
    pump(5)
pump(200)
check("Paste waits for Shift release, then pastes clean Ctrl+V", entry.get_text() == "pasted by ferry",
      f"{(call_done[0] - t) * 1000:.0f} ms; entry={entry.get_text()!r}" if call_done else "no reply")

# 10. Clipboard set by a normal app (GTK) is reported with its source app.
signals.clear()
editor.present()
pump(300)
Gtk.Clipboard.get_default(editor.get_display()).set_text("copied in editor", -1)
pump(400)
changed = [s for s in signals if s[1] == "ClipboardChanged"]
check("ClipboardChanged for an app's own copy", bool(changed), str(changed[-1][2]) if changed else "")

# 11. Disable and enable the extension (what a screen lock does): registration is dropped.
call("DisableExtension", GLib.Variant("(s)", (UUID,)), "(b)", "org.gnome.Shell.Extensions", "/org/gnome/Shell")
pump(300)
call("EnableExtension", GLib.Variant("(s)", (UUID,)), "(b)", "org.gnome.Shell.Extensions", "/org/gnome/Shell")
pump(300)
try:
    call("GetMimeTypes")
    check("registration is dropped after disable/enable", False)
except GLib.Error:
    check("registration is dropped after disable/enable", True)
check("Register works again after enable", call("Register", GLib.Variant("(u)", (1,)), "(u)") == (1,))

failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} passed", flush=True)
sys.exit(1 if failed else 0)
