# Linux RustKit chrome: staged plan

Owner: Talos. Review of record per stage: Castor (R1 at the exact head SHA). Merge: Atlas. A stage's author never merges it.
Direction: Pete, 2026-11-06 ("Build the RustKit-drawn chrome on X11 at first opportunity"; "the story similar on Linux" to the macOS all-RustKit chrome).
Status: S0 done (evidence below). **Flipping any Linux default waits for Pete's word.**

## Where Linux is today (corrects the earlier assumption, verbatim)

The view machinery exists, **nothing constructs `ContentWebView::RustKit`, and the Linux `rustkit` feature does nothing visible today.**
`webview.rs` `engine_name()` / `is_rustkit_enabled()` are `cfg(windows, rustkit)` only, and `main.rs` builds the page with wry `build_gtk` unconditionally.
What does exist (Castor's L3 and the S3 sync): `webview_rustkit::RustKitView` builds on Linux (X11 child window of the app window, `ContentThread`, `WindowSurface::attach_x11`); `ContentWebView` has a `RustKit(Arc<RustKitView>)` variant in `content_webview_enum.rs`; viewhost has an Xlib `create_view`/`resize_view`/`pump_messages`; `region.rs`, `painted_boxes` (engine) and `A3-composition.md` came from macOS. What does not exist: `ChromeUi`/`ChromeView`/`LazyPanel` in Linux `main.rs`, any chrome view, any composition, keyboard routing to a chrome, panels as RustKit views.

**Second finding (found while scoping S1a): the Linux viewhost drops every X11 event.** `X11ViewHost::pump_messages` selects the input masks (key, button, motion, focus, exposure) and then reads each event and discards it (`// TODO: Handle events and emit via callback`). A RustKit view on Linux therefore receives no mouse, keyboard, scroll, focus or resize input today. macOS delivers input through its viewhost into a platform-neutral `InputQueue` (`input_queue.rs`, already synced) that the app drains; Linux has the queue but nothing feeding it. A RustKit page that cannot be clicked or typed into is not usable and a chrome certainly is not, so X11 input is its own stage (S1c) before the chrome.

## S0 evidence (done)

A `RustKitView` created on the real app window renders a page in an X11 child (`1280x700+0+104`) under the WebKit chrome bar, with real text, rounded corners and a shadow, on a private Xvfb. Develop `b39da378`. Screenshot `gate-runner/_logs/s0_screen.png`; harness `gate-runner/_logs/s0_harness.patch` (never committed). `load_html` returned ~1.4 s after view creation (not profiled).

## Design: option H (macOS-shaped hybrid)

A tao window with RustKit X11 child views for page, chrome and panels. The WebKit chrome stays the default and the escape hatch until the RustKit chrome passes its checks. Keeping Linux `main.rs` shaped like macOS (`ChromeUi` / `ChromeView` / `LazyPanel`) is what makes the next sync cheap (the S3 sync cost 48 `main.rs` hunks). A pure native X11 shell with no tao/GTK (option N) is a later decision with its own case; **do not build toward it in S1-S5.**

On macOS the chrome is not a special widget: it is the same `RustKitView` as the page, running `ui/chrome.html`, behind `trait ChromeUi { eval, place, take_keyboard, compose(hole), keep_on_top }`; the app only runs script in it and places it. `ui/chrome.html`, `inspector.html`, the settings page and the shelf page are SHARED files: **host them unchanged; platform differences live in the host** (`ChromeUi`, composition, input), never in the pages.

### Composition on X11

macOS first overlaid the chrome with `region = window - hole + overlays` (`A3-composition.md`, "region-only route"): the chrome is opaque inside its region and absent outside it. That maps directly onto the X11 SHAPE extension: the chrome child's bounding shape is the region (rects from the engine's `painted_boxes`, the page hole, overlays while open), and its input shape is the same. viewhost has no XShape today and the `x11` crate has no shape feature; a small FFI to libXext closes that.

**KNOWN GAP, recorded here and in S2's PR body, not debt to chase before S5:** the macOS alpha route (CH3.A3c: shadows and rounded-corner alpha reaching the page) has no X11 child-window equivalent; SHAPE is binary. The only path to real alpha is a separate ARGB top-level composited by the compositor, which is fragile on stacking and focus. Noted, not now. S4's chrome board must **mask or exclude those pixels** rather than count them as misses, or the gate measures the X server, not the chrome.

### Platform facts that are product facts

- This development box is a Wayland (GNOME) session; tao hands out a Wayland handle by default, and `RustKitView` accepts only Xlib/Xcb. A Wayland user who launches without `GDK_BACKEND=x11` gets the WebKit chrome. Every RustKit flag must detect a non-X11 handle, log **one clear line** saying why it did not engage, and fall back to WebKit; that fallback is **tested before S5**.
- Default-off must mean zero cost: with a flag unset, no `RustKitView`, no X child window, no content thread. Each stage carries a test or a startup log line that proves it.
- The flags compose: content on / chrome off = S1a state; content on / chrome on = S2; **chrome on with content off must log and fall back, not crash** (compose, hole and keep_on_top assume two RustKit views).

## Stages (one PR each; S1a and S1b are independent and may rebase on each other)

| Stage | Content | Verified by |
|---|---|---|
| S0 | Prove a `RustKitView` opens in the real window (evidence only) | screenshot, done |
| S1a | Wire `ContentWebView::RustKit` into `main.rs` behind `HIWAVE_RUSTKIT_CONTENT=1` (default off); non-X11 fallback line; timestamped log lines around view creation and first `load_html` (a number for S4, not a profile) | default-off proof; Xvfb run with the flag on; all feature sets build |
| S1c | **X11 input into the viewhost**: translate `ButtonPress/Release`, `MotionNotify`, wheel, `KeyPress/Release` (Xkb), `FocusIn/Out`, `ConfigureNotify`/`Expose` into the `InputQueue` the app already drains (mouse, scroll, text and command keys, focus, resize); the app forwards them to the `RustKitView` methods that exist for it | unit tests of the pure translation (event struct in, input out, no display needed); Xvfb checks with synthetic events (`xdotool`) |
| S1b | Port `ChromeUi` / `ChromeView` / `LazyPanel` with WebKit as the only backend | `Fake`-impl unit tests; behaviour-neutral |
| S2 | RustKit chrome view behind `HIWAVE_RUSTKIT_CHROME=1` (default off): `chrome.html` in a `RustKitView`, IPC both ways, xlib `place`/`keep_on_top`, XShape region with the page hole | Xvfb screenshot; default-off proof; non-X11 fallback; alpha gap in the PR body |
| S3 | Region growth for overlays (dropdowns, find, palette, modals), keyboard routing (`take_keyboard`), hit-test through the shape, cursor; panels as lazy RustKit views | Xvfb interaction checks |
| S4 | Chrome board on Linux: RustKit render of HiWave's own UI HTML vs Chromium 148 of the same HTML (pixelmatch port, `renders` baselines), alpha pixels masked; frame-time check | board numbers; **gates on Castor's L4/L5 glyph cache** (real text cost ~160 ms/frame on Linux today) |
| S5 | Flip the default to the RustKit chrome with `HIWAVE_RUSTKIT_CHROME=0` as the escape hatch | **Pete's word only** |

## Method and constraints

- Real-window checks run on a **private Xvfb** (`:99`, Ubuntu `xvfb` 2:21.1.18, apt-verified, unpacked without root; see `gate-runner/README.md` for the package, sha256 and recipe). Nothing appears on the live screen. The live display only if Pete says so.
- Every harness or gate run sets `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `XDG_CACHE_HOME` under a scratch dir: a test run never touches the saved profile or loads a saved session.
- No live-site work. Logging is shapes-only (no query strings, no IPC bodies): the log helpers ported in S3 apply to everything new.
- No environment-supplied script runs in the privileged chrome in a release build (macOS #907 R1 nit 2).
- Builds take the gate-runner lock and queue behind batches.
- Linux CI: the only workflow is Linux Metrics; its `Reachability ratchet` step has been red on every PR since #85 (standing exception).
- PR bodies list anything deleted or stubbed.

## Dependencies and risks

- Castor's L4/L5 (LAT2 ignore, Linux glyph cache) touch `rustkit-engine` tests and `rustkit-text` only, not `main.rs` or viewhost `lib.rs`: no file overlap with S1-S3. A chrome is mostly text; it would be correct but laggy until L5 lands, so S4's frame-time check gates on it. If L4 lands first, S2 rebases on it.
- S0 found the view machinery was never wired into the app, and scoping S1a found the viewhost drops all X11 input. Further L3 gaps are likely; each is reported, not worked around.
- S1a (page renders, navigates, syncs title and URL) is deliberately NON-interactive; S1c adds input. Neither is shown as a working browser until both have landed and an Xvfb check clicks and types.
