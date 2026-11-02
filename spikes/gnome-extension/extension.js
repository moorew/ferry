// SPDX-License-Identifier: GPL-2.0-or-later

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const API_VERSION = 1;
const OBJECT_PATH = '/org/gnome/Shell/Extensions/Ferry';
const INTERFACE_NAME = 'org.gnome.Shell.Extensions.Ferry';
const ERROR_PREFIX = `${INTERFACE_NAME}.Error`;

const PRESENT_TIMEOUT_MS = 500;
const MODIFIER_TIMEOUT_MS = 500;
const MODIFIER_POLL_MS = 20;
const PASTE_DELAY_MS = 60;

const HELD_MODIFIERS =
    Clutter.ModifierType.SHIFT_MASK |
    Clutter.ModifierType.CONTROL_MASK |
    Clutter.ModifierType.MOD1_MASK |
    Clutter.ModifierType.MOD4_MASK |
    Clutter.ModifierType.SUPER_MASK |
    Clutter.ModifierType.HYPER_MASK |
    Clutter.ModifierType.META_MASK;

const SHORTCUT_KEYS = {
    'picker': 'picker-shortcut',
    'paste-plain': 'paste-plain-shortcut',
};

const KEYSTROKES = {
    'ctrl-v': [Clutter.KEY_Control_L, Clutter.KEY_v],
    'ctrl-shift-v': [Clutter.KEY_Control_L, Clutter.KEY_Shift_L, Clutter.KEY_v],
    'shift-insert': [Clutter.KEY_Shift_L, Clutter.KEY_Insert],
};

const INTERFACE_XML = `
<node>
  <interface name="${INTERFACE_NAME}">
    <method name="Register">
      <arg type="u" name="client_api" direction="in"/>
      <arg type="u" name="extension_api" direction="out"/>
    </method>
    <method name="GetMimeTypes">
      <arg type="as" name="mimetypes" direction="out"/>
    </method>
    <method name="GetContent">
      <arg type="s" name="mimetype" direction="in"/>
      <arg type="ay" name="data" direction="out"/>
    </method>
    <method name="SetContent">
      <arg type="s" name="mimetype" direction="in"/>
      <arg type="ay" name="data" direction="in"/>
    </method>
    <method name="SetShortcut">
      <arg type="s" name="action" direction="in"/>
      <arg type="as" name="accelerators" direction="in"/>
    </method>
    <method name="PresentPicker">
      <arg type="u" name="pid" direction="in"/>
      <arg type="s" name="title" direction="in"/>
    </method>
    <method name="Paste">
      <arg type="s" name="keystroke" direction="in"/>
      <arg type="b" name="activate_previous" direction="in"/>
    </method>
    <signal name="ClipboardChanged">
      <arg type="as" name="mimetypes"/>
      <arg type="s" name="source_app_id"/>
    </signal>
    <signal name="ShortcutActivated">
      <arg type="s" name="action"/>
      <arg type="s" name="focused_app_id"/>
    </signal>
  </interface>
</node>`;

function getSeat() {
    const backend = global.stage.context?.get_backend() ?? Clutter.get_default_backend();
    return backend.get_default_seat();
}

function appIdForWindow(window) {
    if (!window)
        return '';
    const app = Shell.WindowTracker.get_default().get_window_app(window);
    const id = app?.get_id() ?? '';
    if (id.startsWith('window:'))
        return '';
    return id.replace(/\.desktop$/, '');
}

class FerryService {
    constructor(settings) {
        this._settings = settings;
        this._client = null;
        this._clientPid = 0;
        this._previousWindow = null;
        this._pendingPresent = null;
        this._shortcutTime = 0;
        this._sources = new Set();
        this._clipboard = St.Clipboard.get_default();
        this._keyboard = getSeat().create_virtual_device(
            Clutter.InputDeviceType.KEYBOARD_DEVICE);

        this._dbus = Gio.DBusExportedObject.wrapJSObject(INTERFACE_XML, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);

        global.display.get_selection().connectObject('owner-changed',
            (selection, type, source) => this._onOwnerChanged(type, source), this);
        global.display.connectObject('notify::focus-window',
            () => this._onFocusChanged(), this);

        for (const [action, key] of Object.entries(SHORTCUT_KEYS)) {
            Main.wm.addKeybinding(key, this._settings,
                Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
                Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW,
                () => this._onShortcut(action));
        }
    }

    destroy() {
        for (const key of Object.values(SHORTCUT_KEYS))
            Main.wm.removeKeybinding(key);

        global.display.get_selection().disconnectObject(this);
        global.display.disconnectObject(this);
        this._setPreviousWindow(null);
        this._cancelPendingPresent();

        for (const id of this._sources)
            GLib.Source.remove(id);
        this._sources.clear();

        this._dbus.unexport();
        this._dbus = null;
        this._keyboard = null;
        this._clipboard = null;
        this._settings = null;
    }

    async RegisterAsync([clientApi], invocation) {
        const sender = invocation.get_sender();
        try {
            const reply = await Gio.DBus.session.call(
                'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                'GetConnectionUnixProcessID', new GLib.Variant('(s)', [sender]),
                new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, -1, null);
            const [pid] = reply.deepUnpack();
            const info = await Gio.File.new_for_path(`/proc/${pid}/exe`).query_info_async(
                'standard::symlink-target', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS,
                GLib.PRIORITY_DEFAULT, null);
            const exe = info.get_symlink_target();

            if (!this._settings?.get_strv('allowed-clients').includes(exe)) {
                invocation.return_dbus_error(`${ERROR_PREFIX}.AccessDenied`,
                    `${exe} is not in allowed-clients`);
                return;
            }
            this._client = sender;
            this._clientPid = pid;
            console.log(`Ferry: registered ${exe} (client API ${clientApi})`);
            invocation.return_value(new GLib.Variant('(u)', [API_VERSION]));
        } catch (e) {
            invocation.return_dbus_error(`${ERROR_PREFIX}.AccessDenied`, e.message);
        }
    }

    GetMimeTypesAsync(params, invocation) {
        if (!this._checkCaller(invocation))
            return;
        const mimetypes = this._clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD);
        invocation.return_value(new GLib.Variant('(as)', [mimetypes]));
    }

    GetContentAsync([mimetype], invocation) {
        if (!this._checkCaller(invocation))
            return;
        this._clipboard.get_content(St.ClipboardType.CLIPBOARD, mimetype, (clipboard, bytes) => {
            const data = bytes
                ? GLib.Variant.new_from_bytes(new GLib.VariantType('ay'), bytes, true)
                : new GLib.Variant('ay', []);
            invocation.return_value(GLib.Variant.new_tuple([data]));
        });
    }

    SetContentAsync([mimetype, data], invocation) {
        if (!this._checkCaller(invocation))
            return;
        this._clipboard.set_content(St.ClipboardType.CLIPBOARD, mimetype, new GLib.Bytes(data));
        invocation.return_value(null);
    }

    SetShortcutAsync([action, accelerators], invocation) {
        if (!this._checkCaller(invocation))
            return;
        const key = SHORTCUT_KEYS[action];
        if (!key) {
            invocation.return_dbus_error(`${ERROR_PREFIX}.InvalidArgs`, `unknown action ${action}`);
            return;
        }
        this._settings.set_strv(key, accelerators);
        invocation.return_value(null);
    }

    PresentPickerAsync([pid, title], invocation) {
        if (!this._checkCaller(invocation))
            return;
        invocation.return_value(null);

        const window = global.get_window_actors()
            .map(actor => actor.meta_window)
            .find(w => w.get_pid() === pid && w.get_title() === title);
        if (window) {
            this._present(window);
            return;
        }

        this._cancelPendingPresent();
        this._pendingPresent = {pid, title};
        global.display.connectObject('window-created',
            (display, w) => this._onWindowCreated(w), this._pendingPresent);
        this._pendingPresent.timeout = this._addTimeout(PRESENT_TIMEOUT_MS, () => {
            this._pendingPresent.timeout = 0;
            this._cancelPendingPresent();
            return GLib.SOURCE_REMOVE;
        });
    }

    PasteAsync([keystroke, activatePrevious], invocation) {
        if (!this._checkCaller(invocation))
            return;
        const keys = KEYSTROKES[keystroke];
        if (!keys) {
            invocation.return_dbus_error(`${ERROR_PREFIX}.InvalidArgs`,
                `unknown keystroke ${keystroke}`);
            return;
        }

        const deadline = GLib.get_monotonic_time() + MODIFIER_TIMEOUT_MS * 1000;
        this._addTimeout(MODIFIER_POLL_MS, () => {
            const [, , mods] = global.get_pointer();
            if (mods & HELD_MODIFIERS) {
                if (GLib.get_monotonic_time() < deadline)
                    return GLib.SOURCE_CONTINUE;
                invocation.return_dbus_error(`${ERROR_PREFIX}.ModifiersHeld`,
                    'modifier keys are still held');
                return GLib.SOURCE_REMOVE;
            }

            if (activatePrevious && this._previousWindow)
                Main.activateWindow(this._previousWindow);
            this._addTimeout(PASTE_DELAY_MS, () => {
                this._sendKeys(keys);
                invocation.return_value(null);
                return GLib.SOURCE_REMOVE;
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    _checkCaller(invocation) {
        if (this._client && invocation.get_sender() === this._client)
            return true;
        invocation.return_dbus_error(`${ERROR_PREFIX}.AccessDenied`, 'call Register first');
        return false;
    }

    _emit(name, signature, args) {
        if (!this._client)
            return;
        Gio.DBus.session.emit_signal(this._client, OBJECT_PATH, INTERFACE_NAME,
            name, new GLib.Variant(signature, args));
    }

    _onOwnerChanged(type, source) {
        if (type !== Meta.SelectionType.SELECTION_CLIPBOARD || !source)
            return;
        const mimetypes = this._clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD);
        const appId = appIdForWindow(global.display.focus_window);
        this._emit('ClipboardChanged', '(ass)', [mimetypes, appId]);
    }

    _onShortcut(action) {
        const focused = global.display.focus_window;
        if (action === 'picker') {
            this._shortcutTime = GLib.get_monotonic_time();
            if (focused && focused.get_pid() !== this._clientPid)
                this._setPreviousWindow(focused);
        }
        this._emit('ShortcutActivated', '(ss)', [action, appIdForWindow(focused)]);
    }

    _onFocusChanged() {
        const focused = global.display.focus_window;
        if (!this._shortcutTime || focused?.get_pid() !== this._clientPid)
            return;
        const ms = (GLib.get_monotonic_time() - this._shortcutTime) / 1000;
        console.log(`Ferry: picker focused ${ms.toFixed(1)} ms after the shortcut`);
        this._shortcutTime = 0;
    }

    _onWindowCreated(window) {
        if (window.get_pid() !== this._pendingPresent.pid)
            return;
        window.connectObject('shown', () => {
            window.disconnectObject(this);
            if (window.get_title() !== this._pendingPresent?.title)
                return;
            this._cancelPendingPresent();
            this._present(window);
        }, this);
    }

    _cancelPendingPresent() {
        if (!this._pendingPresent)
            return;
        global.display.disconnectObject(this._pendingPresent);
        if (this._pendingPresent.timeout) {
            GLib.Source.remove(this._pendingPresent.timeout);
            this._sources.delete(this._pendingPresent.timeout);
        }
        this._pendingPresent = null;
    }

    _present(window) {
        const monitor = this._previousWindow?.get_monitor() ?? global.display.get_current_monitor();
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const rect = window.get_frame_rect();
        window.move_frame(true,
            area.x + Math.round((area.width - rect.width) / 2),
            area.y + Math.round((area.height - rect.height) / 2));
        window.make_above();
        Main.activateWindow(window);
    }

    _setPreviousWindow(window) {
        this._previousWindow?.disconnectObject(this);
        this._previousWindow = window;
        window?.connectObject('unmanaged', () => this._setPreviousWindow(null), this);
    }

    _sendKeys(keys) {
        const time = GLib.get_monotonic_time();
        for (const key of keys)
            this._keyboard.notify_keyval(time, key, Clutter.KeyState.PRESSED);
        for (const key of [...keys].reverse())
            this._keyboard.notify_keyval(time, key, Clutter.KeyState.RELEASED);
    }

    _addTimeout(ms, callback) {
        const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            const result = callback();
            if (result === GLib.SOURCE_REMOVE)
                this._sources.delete(id);
            return result;
        });
        this._sources.add(id);
        return id;
    }
}

export default class FerryExtension extends Extension {
    enable() {
        this._service = new FerryService(this.getSettings());
    }

    disable() {
        this._service.destroy();
        this._service = null;
    }
}
