"""End-to-end test of the installed TailCopy app in a headless GNOME Shell.

Drives the real app like a user: copy in an app, open the picker with the shortcut,
paste with Enter, Shift+Enter and search, the paste-plain shortcut, pause and resume
from the CLI, a password manager's copy, and a lock-style extension restart.
"""
import json
import subprocess
import time

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

conn = Gio.bus_get_sync(Gio.BusType.SESSION)
results = []
APP_LOG = "/tmp/app.log"


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


def shell_eval(code):
    r = conn.call_sync("org.gnome.Shell", "/org/gnome/Shell", "org.gnome.Shell", "Eval",
                       GLib.Variant("(s)", (code,)), GLib.VariantType("(bs)"), 0, 5000, None)
    return r.unpack()[1]


KEYS = """
const C = imports.gi.Clutter;
const seat = (global.stage.context?.get_backend() ?? C.get_default_backend()).get_default_seat();
globalThis._kb ??= seat.create_virtual_device(C.InputDeviceType.KEYBOARD_DEVICE);
const t = imports.gi.GLib.get_monotonic_time();
const keys = [%s];
for (const k of keys) globalThis._kb.notify_keyval(t, k, C.KeyState.PRESSED);
for (const k of keys.reverse()) globalThis._kb.notify_keyval(t, k, C.KeyState.RELEASED);
'ok'
"""


def press(keys):
    shell_eval(KEYS % keys)


def type_text(text):
    for ch in text:
        press(f"C.unicode_to_keysym({ord(ch)})")
        pump(15)


def log():
    with open(APP_LOG, errors="replace") as f:
        return f.read()


def wait_log(text, timeout=4.0, after=0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if text in log()[after:]:
            return True
        pump(25)
    return False


def focus_window_title():
    return shell_eval("global.display.focus_window?.get_title() ?? ''").strip('"')


def wait_focus(title, timeout=3.0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if focus_window_title() == title:
            return True
        pump(25)
    return False


def read_clipboard(display, mime):
    out = []

    def done(cb, res):
        try:
            stream, _ = cb.read_finish(res)
            out.append(bytes(stream.read_bytes(1 << 20, None).get_data()))
        except GLib.Error as e:
            out.append(e.message)

    display.get_clipboard().read_async([mime], 0, None, done)
    end = time.monotonic() + 2
    while not out and time.monotonic() < end:
        pump(10)
    return out[0] if out else None


def copy_union(display, parts):
    providers = [Gdk.ContentProvider.new_for_bytes(m, GLib.Bytes.new(d.encode())) for m, d in parts]
    display.get_clipboard().set_content(Gdk.ContentProvider.new_union(providers))


def screenshot(name):
    try:
        conn.call_sync("org.gnome.Shell.Screenshot", "/org/gnome/Shell/Screenshot",
                       "org.gnome.Shell.Screenshot", "Screenshot",
                       GLib.Variant("(bbs)", (False, False, f"/tmp/{name}.png")), None, 0, 5000, None)
    except GLib.Error as e:
        print(f"(screenshot {name} unavailable: {e.message})")


def tailcopy(*args):
    out = subprocess.run(["/usr/bin/tailcopy", *args], capture_output=True, text=True, timeout=10)
    return out.stdout.strip()


# Shell ready, overview closed.
deadline = time.monotonic() + 40
while time.monotonic() < deadline:
    state = shell_eval("JSON.stringify([Main.layoutManager._startingUp, Main.overview.visible, Main.actionMode])")
    if state == '"[false,false,1]"':
        break
    if state.startswith('"[true') and time.monotonic() > deadline - 30:
        shell_eval("Main.layoutManager._startupAnimationComplete(); 'ok'")
    if state.startswith('"[false,true'):
        shell_eval("Main.overview.hide(); 'ok'")
    pump(250)

app = subprocess.Popen(["/usr/bin/tailcopy", "--autostart"], stdout=open(APP_LOG, "w"),
                       stderr=subprocess.STDOUT)
check("app registers with the extension (allowed client /usr/bin/tailcopy)",
      wait_log("registered with the GNOME extension", 20))

Gtk.init()
editor = Gtk.Window(title="Editor")
entry = Gtk.Entry()
editor.set_child(entry)
editor.present()
pump(500)
wait_focus("Editor")
display = editor.get_display()

# Copy formatted text, then plain text.
mark = len(log())
copy_union(display, [("text/html", "<b>bold</b> and <i>italic</i>"),
                     ("text/plain;charset=utf-8", "bold and italic")])
check("formatted copy is captured", wait_log("captured", 4, mark))
pump(300)
mark = len(log())
copy_union(display, [("text/plain;charset=utf-8", "second clip")])
check("plain copy is captured", wait_log("captured", 4, mark))
pump(300)

# Picker: newest first; Down, Enter pastes the formatted clip into the editor.
mark = len(log())
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
check("picker opens and takes focus", wait_focus("TailCopy Picker"), focus_window_title())
pump(400)
screenshot("picker")
press("C.KEY_Down")
pump(150)
press("C.KEY_Return")
check("Enter pastes into the previous app", wait_log("paste sent", 4, mark))
pump(500)
check("editor received the formatted clip's text", entry.get_text() == "bold and italic", repr(entry.get_text()))
check("HTML is still on the clipboard after the picker hid",
      read_clipboard(display, "text/html") == b"<b>bold</b> and <i>italic</i>")

# Shift+Enter on the (now first) formatted clip offers plain text only.
entry.set_text("")
mark = len(log())
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
wait_focus("TailCopy Picker")
pump(300)
press("C.KEY_Shift_L, C.KEY_Return")
wait_log("paste sent", 4, mark)
pump(500)
check("Shift+Enter pastes plain text", entry.get_text() == "bold and italic", repr(entry.get_text()))
html_after = read_clipboard(display, "text/html")
check("plain paste offers no HTML", not isinstance(html_after, bytes) or html_after == b"", repr(html_after)[:60])

# Search then Enter.
entry.set_text("")
mark = len(log())
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
wait_focus("TailCopy Picker")
pump(300)
type_text("second")
pump(400)
screenshot("picker-search")
press("C.KEY_Return")
wait_log("paste sent", 4, mark)
pump(500)
check("search then Enter pastes the match", entry.get_text() == "second clip", repr(entry.get_text()))

# Paste-plain shortcut: what's on the clipboard, as plain text.
entry.set_text("")
mark = len(log())
copy_union(display, [("text/html", "<u>under</u>"), ("text/plain;charset=utf-8", "under")])
wait_log("captured", 4, mark)
pump(300)
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_Shift_L, C.KEY_v")
pump(800)
check("paste-plain shortcut pastes", entry.get_text() == "under", repr(entry.get_text()))

# CLI: status, pause, resume.
status = json.loads(tailcopy("status") or "{}")
check("`tailcopy status` reports JSON", status.get("session") == "gnome" and status.get("clipCount", 0) >= 3,
      f"session={status.get('session')} clips={status.get('clipCount')}")
tailcopy("pause")
pump(300)
mark = len(log())
copy_union(display, [("text/plain;charset=utf-8", "copied while paused")])
pump(800)
check("nothing is captured while paused", "captured" not in log()[mark:])
tailcopy("resume")
pump(300)
mark = len(log())
copy_union(display, [("text/plain;charset=utf-8", "after resume")])
check("capture resumes", wait_log("captured", 4, mark))

# A password manager's copy is skipped.
mark = len(log())
copy_union(display, [("text/plain;charset=utf-8", "hunter2"), ("x-kde-passwordManagerHint", "secret")])
check("password manager copies are skipped", wait_log("skipped a clip marked secret", 4, mark))

# Lock and unlock stand-in.
mark = len(log())
for method in ("DisableExtension", "EnableExtension"):
    conn.call_sync("org.gnome.Shell", "/org/gnome/Shell", "org.gnome.Shell.Extensions", method,
                   GLib.Variant("(s)", ("tailcopy@moorew.github.io",)), None, 0, 5000, None)
    pump(400)
check("app re-registers after the extension restarts", wait_log("registered with the GNOME extension", 5, mark))
entry.set_text("")
editor.present()
pump(300)
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
wait_focus("TailCopy Picker")
pump(300)
press("C.KEY_Return")
pump(800)
check("shortcut works after re-registering", entry.get_text() == "after resume", repr(entry.get_text()))

check("no clip content in the log", "hunter2" not in log() and "bold and italic" not in log())

app.terminate()
failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} passed", flush=True)
