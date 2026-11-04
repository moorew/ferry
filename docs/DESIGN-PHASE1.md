# Design Phase 1: tokens, fonts, theming

3 November 2026. Branch `design` (off `m1`). Handoff section 2, decision D-021.

## What shipped

- **One token source.** `app/src/app.css` now imports `design/tokens/tokens.css` and
  defines nothing of its own. Every component reads `--tc-*` variables; the old
  `--bg`, `--fg`, `--hairline` and friends are gone.
- **Fonts.** Bricolage Grotesque (page and state titles), Figtree (everything else),
  JetBrains Mono (code), bundled from `design/fonts/variable` by Vite, nothing fetched.
  Commissioner is removed. The three `OFL-*.txt` files ship as bundle resources under
  `licenses/`, and Settings > About credits the fonts.
- **Theme.** Follows the OS and switches live. New **Settings > General > Appearance**:
  System, Light, Dark. It's `general.theme` in `config.toml` (SPEC 11 updated); the
  app sets the native window theme (`AppHandle::set_theme`, so title bars match) and
  `data-theme` on both windows, and the picker picks up a change without reopening.
- **Restyled with the new tokens, not yet redesigned:** sidebar on `sunk`, the
  violet capturing dot with its halo (hollow when paused), the 44 × 26 accent toggle,
  accent focus on fields and the shortcut recorder, 2 px accent focus ring with a
  surface gap everywhere, 56 px history rows and 46 px picker rows, picker search at
  17 px, picker open animation from the motion tokens (140 ms fade and scale 0.98;
  reduced motion: 120 ms fade only).
- **Guard.** `npm run check:tokens` fails on any literal colour (`#hex`, `rgb()`,
  `hsl()`) in `app/src` outside the mock backend. Added to CI and to CLAUDE.md's
  front-end commands.

## Tokens I added (handoff rule: add a token first, and say so)

In both `tokens.css` and `tokens.json` unless noted:

| Token | Value | Why |
| --- | --- | --- |
| `--tc-knob` | `#FFFFFF`, both themes | The toggle spec says "white knob"; no white token existed in dark |
| `--tc-text-page`, `-state`, `-card`, `-search`, `-setting`, `-body`, `-code`, `-meta`, `-keycap` | 34, 28, 26, 17, 15, 14, 13, 12, 11 px | Already in `tokens.json` (`font.style`), missing from `tokens.css` |
| `--tc-row-history`, `--tc-row-tile`, `--tc-row-picker`, `--tc-row-tile-picker`, `--tc-toggle-w`, `--tc-toggle-h` | 56, 36, 46, 30, 44, 26 px | Already in `tokens.json` (`size`), missing from `tokens.css` |

## Old values with no direct token, and what they became

| Old | Used for | Now |
| --- | --- | --- |
| `--warn` (amber) | Paused chip in the picker | `--tc-ink-2` (paused is a state, not a problem) |
| `--warn` | Paused dot in the sidebar | Hollow dot, `--tc-ink-2` border (handoff status card) |
| `--warn` | Warning text and icon in Settings and first run | `--tc-danger` (the design has one warning colour) |
| `--bg` as a fill behind controls | Buttons, fields | `.btn` transparent, `.field` `--tc-surface` |
| `--fg` as a fill | Selected nav, primary button, progress, tokens | `--tc-selected` with `--tc-on-selected` text |
| `--text-lg` (16) | Lede, brand, empty title | `--tc-text-setting` (15); picker search `--tc-text-search` (17) |
| `--text-xl` (20) | History and Settings titles | `--tc-text-page` (34) in Bricolage 800 |
| `rgb(0 0 0 / 30%)` | Toggle knob shadow | `--tc-shadow-card` |
| `border-radius` 2 / 4 / 6 px / 50% / 999 px | Progress, kbd, thumbnails, dots, pills | `--tc-r-dot` / `--tc-r-keycap` / `--tc-r-keycap` / `--tc-r-pill` / `--tc-r-pill` |
| `120ms`, `150ms` transitions | Toggle, "Saved" fade | `--tc-dur-fast` with `--tc-ease-out` |
| Picker open, 100 ms, scale 0.985 | | `--tc-dur-enter`, scale 0.98, `--tc-ease-out` |

Font weights stay literal: the handoff's rule covers colour, radius, size and duration.

## Checks

| Check | Result |
| --- | --- |
| `npm run check`, `lint`, `check:tokens`, `build` | Pass. `check:tokens` finds no literal colour |
| `cargo fmt --check` | Pass |
| `cargo clippy --workspace --all-targets -D warnings` (Linux, in Docker) | Pass |
| `cargo clippy -p tailcopy --target x86_64-pc-windows-msvc` and `aarch64-apple-darwin` (stubbed C toolchain) | Pass |
| `cargo test --workspace` | Pass (new `theme_round_trips`, bad `theme` value rejected) |
| Light, dark and forced Light on a dark OS, in Chromium against the mock backend | Renders; switching is live; System clears the override |

Screenshots, before and after, both themes: `design/qa/phase1/`. They're taken in
headless Chromium with the mock backend (`design/qa/shoot.mjs`), so they show the
styles, not the real webviews.

## Not verified

- **The real webviews.** WebView2, WKWebView and WebKitGTK should render the same,
  but I haven't seen them. Worth a look on Windows: the variable fonts and the
  Mica material under the new lilac background.
- **Native title bar follows the setting** on Windows and macOS: `set_theme` is the
  documented call, untested on hardware.
- **Picker transparency on Windows/macOS** with the new 22 px radius: unchanged code,
  but the larger radius may show the corners differently over Mica.

## Notes for later phases

- The detail pane's preview box is still a large empty block for short items
  (handoff fix 4); that's Phase 4.
- The picker is still 640 × 420 with 46 px rows, so seven rows show. Phase 5 makes it
  960 × 640.
- Bricolage Grotesque now ships as the static ExtraBold (91 KB, was 408 KB), owner decision.
- `tokens.css` uses `font-display: swap`. The picker's webview stays loaded, so the
  fonts are cached after first launch; I'll check for a flash of fallback on first open
  in Phase 5.
