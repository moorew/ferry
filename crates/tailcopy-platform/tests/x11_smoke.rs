//! X11 backend end to end, against a real X server with an EWMH window manager:
//! `xvfb-run -a sh -c 'openbox & sleep 1; cargo test -p tailcopy-platform --test x11_smoke'`.
//! Needs `xclip`. Skips when there is no display, as in CI.

#[cfg(target_os = "linux")]
fn main() {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use gtk::prelude::*;
    use tailcopy_platform::{Keystroke, Marker, PasteResult, Write, linux::gtk_write, x11};

    if std::env::var_os("DISPLAY").is_none() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        eprintln!("x11_smoke: no X11 display, skipped");
        return;
    }
    gtk::init().expect("GTK");

    // Keeps GTK serving the clipboard and handling input while `f` runs elsewhere.
    fn off_main<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        loop {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
            if let Ok(v) = rx.try_recv() {
                return v;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn pump(d: Duration) {
        let end = Instant::now() + d;
        while Instant::now() < end {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    let changes = x11::watch().expect("watch");

    // 1. Our own multi-format write reads back, and the watcher sees it.
    let landed = gtk_write(&Write {
        text: Some("hello x11"),
        html: Some("<b>hello</b> x11"),
        ..Write::default()
    });
    assert_eq!(landed.landed, ["text/plain", "text/html"]);
    let got = off_main(move || {
        changes
            .recv_timeout(Duration::from_secs(2))
            .expect("change event");
        (x11::read().expect("read"), changes)
    });
    let (captured, changes) = got;
    assert_eq!(captured.text.as_deref(), Some("hello x11"));
    assert_eq!(captured.html.as_deref(), Some("<b>hello</b> x11"));
    assert_eq!(captured.marker, Marker::None);
    println!("ok: GTK write, XFixes watch, read back");

    // 2. A password manager's hint keeps the copy out (D-020).
    let mut xclip = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "x-kde-passwordManagerHint"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("xclip");
    {
        use std::io::Write as _;
        xclip.stdin.take().unwrap().write_all(b"secret").unwrap();
    }
    // xclip keeps serving from a child of its own once stdin closes.
    xclip.wait().expect("xclip");
    changes
        .recv_timeout(Duration::from_secs(2))
        .expect("change event");
    let captured = off_main(|| x11::read().expect("read"));
    assert!(matches!(captured.marker, Marker::DontSave(_)));
    assert!(captured.text.is_none());
    println!("ok: password manager hint");

    // 3. Another app has focus; the picker-like window maps and present() focuses it,
    //    then it takes a pasted Ctrl+V.
    let other = gtk::Window::new(gtk::WindowType::Toplevel);
    other.set_title("Other app");
    other.show_all();
    pump(Duration::from_millis(1500));
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("TailCopy smoke");
    let entry = gtk::Entry::new();
    window.add(&entry);
    window.show_all();
    off_main(|| x11::present("TailCopy smoke"));
    pump(Duration::from_millis(300));
    assert_eq!(active_title(), "TailCopy smoke");
    println!("ok: present() focused our window");

    entry.grab_focus();
    gtk_write(&Write {
        text: Some("pasted by XTest"),
        ..Write::default()
    });
    let result = off_main(|| x11::paste(None, Keystroke::CtrlV));
    assert_eq!(result, PasteResult::Pasted);
    pump(Duration::from_millis(500));
    assert_eq!(entry.text().as_str(), "pasted by XTest");
    println!("ok: Ctrl+V through XTest");

    // 4. The active window is ours, so there is no previous app to return to.
    assert!(x11::foreground().is_none());
    println!("ok: own window isn't a paste target");

    // 5. present() alone switches between windows that are already mapped.
    off_main(|| x11::present("Other app"));
    pump(Duration::from_millis(300));
    assert_eq!(active_title(), "Other app");
    off_main(|| x11::present("TailCopy smoke"));
    pump(Duration::from_millis(300));
    assert_eq!(active_title(), "TailCopy smoke");
    println!("ok: present() switches focus");
}

#[cfg(target_os = "linux")]
fn active_title() -> String {
    use std::process::Command;
    let xprop = |args: &[&str]| {
        let out = Command::new("xprop").args(args).output().expect("xprop");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let active = xprop(&["-root", "_NET_ACTIVE_WINDOW"]);
    let id = active
        .rsplit(' ')
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    let name = xprop(&["-id", &id, "_NET_WM_NAME"]);
    name.split('"').nth(1).unwrap_or_default().to_owned()
}

#[cfg(not(target_os = "linux"))]
fn main() {}
