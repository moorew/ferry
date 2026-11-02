"""End-to-end test of the gnome-client spike against the extension spike."""
import os
import subprocess
import time

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

conn = Gio.bus_get_sync(Gio.BusType.SESSION)
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


def eval_js(code):
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
    eval_js(KEYS % keys)


def client_log():
    with open("/tmp/client.log") as f:
        return f.read()


def wait_log(text, timeout=3.0, after=0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if text in client_log()[after:]:
            return True
        pump(20)
    return False


def read_clipboard(display, mime):
    out = []

    def done(cb, res):
        try:
            stream, _ = cb.read_finish(res)
            data = stream.read_bytes(1 << 20, None).get_data()
            out.append(bytes(data))
        except GLib.Error as e:
            out.append(e.message)

    display.get_clipboard().read_async([mime], 0, None, done)
    end = time.monotonic() + 2
    while not out and time.monotonic() < end:
        pump(10)
    return out[0] if out else None


# Shell ready, overview closed.
deadline = time.monotonic() + 40
while time.monotonic() < deadline:
    state = eval_js("JSON.stringify([Main.layoutManager._startingUp, Main.overview.visible, Main.actionMode])")
    if state == '"[false,false,1]"':
        break
    if state.startswith('"[true') and time.monotonic() > deadline - 30:
        eval_js("Main.layoutManager._startupAnimationComplete(); 'ok'")
    if state.startswith('"[false,true'):
        eval_js("Main.overview.hide(); 'ok'")
    pump(250)

# Start the Tauri client.
log = open("/tmp/client.log", "w")
client = subprocess.Popen([os.environ.get("TAILCOPY_CLIENT", "/client/tailcopy")], stdout=log, stderr=subprocess.STDOUT)
check("client registers with the extension", wait_log("registered, extension API 1", 15))

# An editor app with a text field.
Gtk.init()
editor = Gtk.Window(title="Editor")
entry = Gtk.Entry()
editor.set_child(entry)
editor.present()
pump(800)
check("editor is focused", editor.is_active())
display = editor.get_display()

# Copy rich content (HTML + plain text in one offer) from the editor.
html = "<b>bold</b> and <i>italic</i>"
text = "bold and italic"
mark = len(client_log())
provider = Gdk.ContentProvider.new_union([
    Gdk.ContentProvider.new_for_bytes("text/html", GLib.Bytes.new(html.encode())),
    Gdk.ContentProvider.new_for_bytes("text/plain;charset=utf-8", GLib.Bytes.new(text.encode())),
])
display.get_clipboard().set_content(provider)
check("client captures text and HTML", wait_log("GetContent text/html", 3, mark))

# Picker shortcut, then Enter: GTK multi-format write, hide, paste into the editor.
mark = len(client_log())
t = time.monotonic()
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
focused = wait_log("picker focused (webview)", 3, mark)
check("picker shows and takes focus", focused, f"{(time.monotonic() - t) * 1000:.0f} ms incl. polling")
pump(200)
mark = len(client_log())
press("C.KEY_Return")
check("Enter pastes into the previous window", wait_log("Paste ctrl-v sent", 3, mark))
pump(400)
check("editor received the text", entry.get_text() == text, repr(entry.get_text()))
check("offer survives the picker hiding (seen by the shell)", wait_log("offer after hide", 3, mark))
got_html = read_clipboard(display, "text/html")
check("HTML is still served after hide", got_html == html.encode(), repr(got_html)[:80])

# Shift+Enter: plain text only.
entry.set_text("")
mark = len(client_log())
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
wait_log("picker focused (webview)", 3, mark)
pump(200)
press("C.KEY_Shift_L, C.KEY_Return")
wait_log("Paste ctrl-v sent", 3, mark)
pump(400)
check("Shift+Enter pastes plain text", entry.get_text() == text, repr(entry.get_text()))
formats = [line for line in client_log()[mark:].splitlines() if "offer after hide" in line]
check("plain offer has no HTML", bool(formats) and "text/html" not in formats[-1], formats[-1][-120:] if formats else "")

# Global paste-plain shortcut (background write through SetContent).
entry.set_text("")
mark = len(client_log())
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_Shift_L, C.KEY_v")
wait_log("Paste ctrl-v sent", 3, mark)
pump(400)
check("paste-plain shortcut pastes", entry.get_text() == text, repr(entry.get_text()))

# Lock and unlock stand-in: the client re-registers by itself.
mark = len(client_log())
for method in ("DisableExtension", "EnableExtension"):
    conn.call_sync("org.gnome.Shell", "/org/gnome/Shell", "org.gnome.Shell.Extensions", method,
                   GLib.Variant("(s)", ("tailcopy@moorew.github.io",)), None, 0, 5000, None)
    pump(300)
check("client re-registers after the extension is re-enabled", wait_log("re-registered", 3, mark))
entry.set_text("")
press("C.KEY_Control_L, C.KEY_Alt_L, C.KEY_v")
wait_log("picker focused (webview)", 3, mark)
pump(200)
press("C.KEY_Return")
wait_log("Paste ctrl-v sent", 3, mark)
pump(400)
check("shortcuts work after re-registering", entry.get_text() == text, repr(entry.get_text()))

client.terminate()
failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} passed", flush=True)
