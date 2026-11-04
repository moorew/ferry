# Design QA

Screenshots of every screen in light and dark, at the board sizes, from headless
Chromium against the mock backend (`app/src/lib/mock.ts`, mock data shaped like the
boards). Taken with `design/qa/shoot.mjs`-style scripts; `final/` is the end state
after Phase 7. The real webviews (WebView2, WKWebView, WebKitGTK) may differ slightly,
mostly in font rendering and window materials.

| Screen | Board | Files (`final/`) | Verdict |
| --- | --- | --- | --- |
| History | History, light | `history-light`, `history-dark` | **Matches.** Differs on purpose: no Paste in the detail card (the window has no app to paste into), no "From other devices" chip until M2, no "Password skipped" rows (D-021), lockup instead of mark plus text |
| History, Images | History, dark, Images view | `history-images-*` | **Matches.** 64 × 44 thumbnails, aspect-correct preview. "Kept on mac-mini, over the 10 MB limit" can't happen (local-only clips never reach other devices); local ones say "Kept on this device" |
| Quick picker | Quick picker | `picker-*` | **Matches.** Differs on purpose: no "Pasted into…" toast (D-021), device name shown next to the dot, "All devices" is a label until M2 |
| Pinned shelf | Pinned shelf | `pinned-*` | **Matches.** |
| Settings, Tailnet | Settings | `settings-tailnet-*` | **Differs on purpose:** device list, Send/Receive, Images/Formatted toggles and Relay wait for M2/M3; the section says so instead of showing dead switches |
| Settings, General | (no board) | `settings-general-*` | Built in the Settings board's style |
| Empty history | States | `state-empty-*` | **Matches.** |
| Tailscale not running | States | `state-offline-*` | **Matches** (inside Settings > Tailnet). "Open Tailscale" shows on Windows and macOS only |
| Pause menu | States | `state-pause-menu-*` | **Matches.** The paused card says "Resumes at …" rather than "Paused for 1 hour" |
| First run | States, "Found your tailnet" | `state-first-run-*` | **Differs on purpose:** "Found your tailnet" needs other devices (M2); today's first run has the new look and the happy character |
| Components | Components | `components-*` (full page) | **Matches.** No private row or "Password skipped" badge (D-021); StatusCard has a third "Not capturing" state |
| Keyboard focus | (handoff 10) | `focus-keyboard-*` | 2 px accent ring with a surface gap on a Pinned card |

No row is "still wrong".

## Accessibility (handoff section 10)

| Check | Result |
| --- | --- |
| Text contrast 4.5:1 | Every token pair the UI uses passes in both themes; text on the six device colours is at least 5.06:1 (light) and 5.7:1 (dark). Script in `docs/DESIGN-PHASE7.md` |
| Device colour never alone | Rows, picker rows, trail, sidebar and Pinned cards show the device name; avatars carry it as label and tooltip |
| Icon-only buttons named | `IconButton` requires a label (accessible name and tooltip) |
| Focus ring everywhere | Global `:focus-visible` ring; picker rows are announced through `aria-activedescendant` on the search (a combobox), so focus can stay in search |
| Reduced motion | Fades only, at most 120 ms, no translate or scale (checked: the toast's transform stays `none`) |
| Hit targets 32 px+ | Chips, footer hints and the Appearance options raised to 32 px; buttons 40 (32 compact) |
| Toasts | `role="status"`, polite, never take focus |

`final/gnome50-real-picker.png` is the real app (WebKitGTK) in the GNOME 50
end-to-end test, not the mock: fonts, size and tokens render as in Chromium.
