# digest-castor

Castor appends one section per session: `## <date> <HH:MM> Castor <package>` with before, after, PRs with SHAs, blockers, and at most three decisions for Atlas.

## 2026-10-29 15:58 Castor S1 + A1

- **Seat / Doorbell (`legion5` `ip -4 addr show` shape)**: `wlp0s20f3 inet 192.168.1.63/24` (Tailscale `tailscale0 inet 100.77.217.43/32`), UDP doorbell `47476`, stream `legion5-efacb6.castor`, `null-daemon-castor.service` active.
- **Before**: `hiwave-linux` `develop` at `c76c2414` (`V1.2.0`, N1 macOS pin `15d2c3a6`, 222 first-parent merges behind `hiwave-macos` `develop` `d16d0f89`); open PR `#84` (`talos/n2-sync-2026-10-15`, SHA `7d0a71a9`, syncing `15d2c3a6..73ae13cb`).
- **After**:
  - Carried `#84` (`7d0a71a9`) and integrated `73ae13cb..d16d0f89` (82 first-parent merges, 35 merge conflicts union-resolved and execution-verified) on branch `castor/n3-sync-2026-10-29`, covering the full macOS SHA range `15d2c3a6..73ae13cb..d16d0f89`.
  - **Workspace tests (`ulimit -n 65536 && cargo test --workspace --no-fail-fast -- --test-threads=1`)**: **3,833 passed, 0 failed, 33 ignored** across 56 non-empty test suites (load `1.49, 1.92, 1.49` -> `1.01, 1.05, 1.07`), plus `206 passed, 0 failed` across `rustkit-engine` `tests/linux_carried_tests.rs` (115) and the 13 other `rustkit-engine` integration test targets (91).
  - **26-case Linux parity campaign (Linux Chrome 148 baselines from `renders-n1`, `pixelmatch`)**: **Linux S1 mean `1.1402%`** vs **Mac `latest-develop.json` (`ff6ad36a`) mean `1.1075%`** (delta **`+0.0327%`**, **`0 / 26` cases over `+1.0 pt` vs Mac**, load `0.51, 1.78, 1.64` -> `0.66, 1.77, 1.64`).
  - **Package `A1` audit**: 15 findings across `15d2c3a6..73ae13cb..d16d0f89` recorded in `trench/castor/audit-castor.md`.
- **PRs**:
  - **PR `#85`** (`https://github.com/hiwavebrowser/hiwave-linux/pull/85`, branch `castor/n3-sync-2026-10-29`, sync commit `346d549c`, ready for R1 from **Argos**).
- **Blockers**: None (awaiting Argos R1 on PR `#85`; no remaining `open` rows in `PLAN-castor.md`).

### Red-Test Proposals for `hiwave-macos` (from Package `A1`)

1. **`AncestorCompound::matches_ancestor` / `complex_alternatives_hold` false-positive match (`crates/rustkit-engine/src/lib.rs:35466,35534`, macOS `d16d0f89` / `#822`)**:
   - *Bug*: `AncestorCompound::matches_ancestor` returns `true` for any `:is()`/`:where()` list group whose `alts.undecidable()` is `true` (i.e., contains at least one complex selector), and `complex_alternatives_hold` tests its compound siblings using `a.matches(tag, classes, id)` without checking `a.attrs` or `a.none_of`.
   - *Red test*: `<div data-theme="light"><p class="child">x</p></div>` styled with `:is([data-theme="dark"], .dark *) > .child { color: rgb(255, 0, 0); }` (or `<div class="light"><p class="child">x</p></div>` styled with `:is(:not(.light), .dark *) > .child { color: rgb(255, 0, 0); }`) — currently matches `.child` even though the attribute/`:not()` compound is false and no `.dark` ancestor exists.
2. **`flex` shorthand keyword and `<flex-basis>` no-op (`crates/rustkit-engine/src/lib.rs:11748` on macOS `d16d0f89`, union-fixed in Linux S1 `346d549c`)**:
   - *Bug*: macOS parses `parts.first()` only via `parse::<f32>()` and falls through on non-numeric single tokens, making `flex: none` (`0 0 auto`), `flex: auto` (`1 1 auto`), `flex: initial` (`0 1 auto`), and `flex: 200px` (`1 1 200px`) complete no-ops on macOS.
   - *Red test*: assert `flex: none` produces `(0.0, 0.0, FlexBasis::Auto)`, `flex: auto` produces `(1.0, 1.0, FlexBasis::Auto)`, `flex: 120px` produces `(1.0, 1.0, FlexBasis::Length(Px(120.0)))`, and `flex: 1 2 3` is rejected without mutating an earlier `flex` declaration.
3. **`writing-mode` validation, inheritance, and `@container (inline-size)` axis selection (`crates/rustkit-engine/src/lib.rs:13196`, `container_query.rs:466`, `crates/rustkit-css/src/lib.rs:2740`, macOS `68f4c8e1` `#782` / `efb2c07b` `#811`)**:
   - *Bug*: `apply_style_property_impl` sets `style.writing_mode_declared = !value.eq_ignore_ascii_case("horizontal-tb")` without validating `value` (so invalid `writing-mode: bogus` taints `@container (inline-size)`), never updates `style.writing_mode`, and `ComputedStyle::inherit_from` does not copy `writing_mode_declared`.
   - *Red test*: `<div style="container-type: inline-size; width: 500px; writing-mode: bogus"><p id="t">x</p></div>` with `@container (inline-size > 400px) { #t { color: rgb(0, 128, 0); } }` — currently fails to match because `writing-mode: bogus` sets `writing_mode_declared = true`.
4. **Media query range direction/arity & invalid range serialization (`crates/rustkit-css/src/media.rs:236,948`, macOS `662b1651` `#818`)**:
   - *Bug*: `range_matches` does not enforce `ops.len() <= 2` or consistent `<`/`>` direction (`(400px < width > 300px)`), and `serialize_in_parens` accepts any `<`/`>`/`=` range containing any CSS ident instead of serializing invalid ranges to `not all`.
   - *Red test*: `window.matchMedia("(400px < width > 300px)")` and `window.matchMedia("(foo < 10px)")` must both report `.matches === false` and `.media === "not all"`.
5. **`console_log::tests::Reset::drop` stale `aliases()` under `--test-threads=1` (`crates/rustkit-engine/src/console_log.rs:483`, macOS `9c37ab09` `#815`, fixed in Linux S1 `346d549c`)**:
   - *Bug*: `Reset::drop` clears `FORCE` and `SINK` but not `aliases()`, so `cargo test -p rustkit-engine --lib console_log::tests -- --test-threads=1` fails on `script_places_inserted_scripts_and_imports_drop_the_query_too` (`inline#1` resolves to `http://127.0.0.1:<port>/` from earlier `load_url` tests).
6. **Case-insensitive `!important` in `@supports` / `CSS.supports` (`crates/rustkit-cssparser/src/lib.rs:210`, macOS `337dd561` `#781`)**:
   - *Bug*: `supports_declaration` calls `.trim_end_matches("!important")` before `.to_ascii_lowercase()`, so `@supports (display: grid !IMPORTANT)` and `CSS.supports("display", "grid !IMPORTANT")` return `false`.
7. **macOS `box-shadow` accumulation, invalid `overflow` reset, and `<link rel="preload stylesheet">` / `href=""` (`crates/rustkit-engine/src/lib.rs:4920,12445,13514`, preserved on Linux in S1)**:
   - *Bug*: on macOS, two matching rules with `box-shadow` append instead of replacing and `box-shadow: none` does not clear; `overflow: hiden` resets an earlier `overflow: hidden` to `visible`; and `<link rel="preload stylesheet">` is skipped while `<link rel="stylesheet" href="">` re-fetches the document URL as CSS.

### Decisions for Atlas

1. **Superseding PR `#84` with PR `#85`**: PR `#85` (`castor/n3-sync-2026-10-29`) carries `#84` (`7d0a71a9`, `15d2c3a6..73ae13cb`) and integrates `73ae13cb..d16d0f89` with all 3,833 single-threaded workspace tests green and the 26-case campaign at `1.1402%` (vs Mac `1.1075%`, `0/26` over `+1.0 pt`). Should `#84` be closed once `#85` passes Argos R1 and merges?
2. **Next Linux packages in `PLAN-castor.md`**: From the `A1` audit (`trench/castor/audit-castor.md`), the top Linux follow-ups are: (a) 16 MiB main-thread stack linker flag on Linux (`crates/hiwave-app/build.rs`) + X11 `WindowSurface` / `ContentThread` wiring (`#770`/`#780` parity), (b) updating `scripts/parity_lib.py` (`--html-file`/`--dump-frame` and the 26 `cases/registry.json` cases) and syncing the stale `crates/hiwave-app/src/ui/{about,chrome,settings,shelf}.html` templates, and (c) ungating the 9 cross-platform headless tests in `match_media_tests.rs`, `computed_content_tests.rs`, and `abspos_percent_height_image_tests.rs`. Which should be queued next?
3. **Cross-porting S1 test/applier fixes to `hiwave-macos`**: Should the 4 fixes landed in Linux S1 (`Compositor::share_device` in `test_compositor()`, `console_log::tests::Reset::drop` clearing `aliases()`, `inspector_page_tests::shown_in` hyphen line-wrap handling, and the `flex` shorthand union fix) plus the `A1` red-test proposals above be queued on the macOS side?

## 2026-10-29 21:21 Castor S2

- **Before**: `hiwave-linux` `develop` at `d8650a71` (with `#85` merged at `f1d2f47f`, synced to `hiwave-macos` `d16d0f89`). `hiwave-macos` `develop` at `3c6b170e` (`#825` `6dc65b89`, `#777` `2ab11616`, `#824` `aa79ec74`, `#826` `831cf6f7`, `#827` `3c6b170e`).
- **After**:
  - Removed the S1 worktree (`/home/petec/repos/hiwave-linux-castor` and its `target` dir) per fresh-checkout rule and created `/home/petec/repos/hiwave-linux-castor-s2` on `castor/s2-sync-2026-10-29`.
  - **Red-first fixes from Prometheus's `#85` R1 (`68200d3a` red -> `12464a45` fix)**:
    1. **`flex` shorthand basis-first (`crates/rustkit-engine/src/lib.rs:12904`, `crates/rustkit-engine/src/linux_carried_tests.rs:1402`)**: Added red assertions for `flex: 10px 2 3` and `flex: 10px 1 0` (plus negative `"2 10px 3"`), then tracked `basis_idx` so only `<number> <basis> <number>` (`basis_idx == Some(1)`) is rejected while `<basis> <grow> <shrink>` parses (`(2.0, 3.0, Length(10.0))` and `(1.0, 0.0, Length(10.0))`).
    2. **`console_log` test alias race (`crates/rustkit-engine/src/console_log.rs:44`)**: Added red test `aliases_do_not_race_across_threads_or_leak_across_force_for_test` (concurrent `Reset` on a second thread + `force_for_test(None)` cleanup on the same thread), then moved test aliases to thread-local `TEST_ALIASES` under `#[cfg(test)]` cleared on `force_for_test(...)`.
  - **Synced `d16d0f89..3c6b170e` (`d70b8fd2`)**: Clean 3-way `git merge-file` across all 19 changed files (`rc = 0` on all 5 shared files; 14 files 100% byte-identical to `3c6b170e`).
  - **Workspace tests (`--test-threads=1`)**: **3,865 passed, 0 failed, 33 ignored** across 56 non-empty suites (`+32` new tests over `S1`; load `0.12, 0.62, 0.42` -> `1.12, 1.16, 1.09`).
  - **26-case Linux parity campaign**: **`1.1410%` mean** vs Mac `latest-develop.json` (`ff6ad36a`) **`1.1075%`** (delta **`+0.0335%`**, **`0 / 26`** cases over `+1.0 pt` vs Mac; load `2.75, 2.84, 1.87` -> `2.48, 2.78, 1.86`).
  - **Audit (`trench/castor/audit-castor.md`)**: Appended 5 `S2` findings (`flex_item_paint_order_tests.rs` & `positioned_paint_order_tests.rs` macOS+headless gate, `FetchPolicy::reserve` budget leak on `validate_shape` failure, `DOMParser` frameset `body`, `insertAdjacentHTML`/`createContextualFragment` on SVG `<title>`/`<script>`/`<style>`, and `z-index: 0` vs `auto` on static flex/grid items).
- **Red-Test Proposals for `hiwave-macos` (`S2` range)**:
  1. **`FetchPolicy::reserve` leaks `max_total_requests` budget on `validate_shape` failure (`crates/rustkit-net/src/policy.rs:605-610`, `#827` `3c6b170e`)**:
     - *Red test*: On a single `FetchPolicy` with `max_total_requests: 1`, call `p.reserve(&bad_trace_req, None)` (returns `Err(Denial::BadRequest(_))`), then call `p.reserve(&valid_get_req, None)` — currently fails with `Err(Denial::BudgetExhausted)` because `self.total` was incremented before `self.validate_shape(req)?` without decrementing on error.
  2. **`DOMParser.parseFromString` frameset document `body` (`crates/rustkit-bindings/src/dom.rs:629,3263`, `#825` `6dc65b89`)**:
     - *Red test*: Evaluate `var p = new DOMParser().parseFromString('<html><head></head><frameset cols="*"><frame src="a"></frameset></html>', 'text/html'); p.body.localName + ':' + p.documentElement.childNodes.length` — currently returns `"body:3"` instead of `"frameset:2"` because `dom.rs:629` appends a synthetic `<body>` when no `<body>` child exists and `dom.rs:3263` searches only for `localName === 'body'`.
  3. **`insertAdjacentHTML` / `createContextualFragment` on SVG `<title>` (`crates/rustkit-bindings/src/dom.rs:604,2576`, `#777` `2ab11616`)**:
     - *Red test*: Evaluate `document.body.innerHTML = '<svg><title></title></svg>'; var t = document.body.firstChild.firstChild; t.insertAdjacentHTML('beforeend', '<tspan>a</tspan>'); t.firstChild.nodeType` — currently returns `3` (Text `"<tspan>a</tspan>"`) instead of `1` (Element `<tspan>`) because `insertAdjacentHTML` parses into a detached HTML-namespace `<title>` holder where `is_in_html_content` returns `true`.
- **PRs**:
  - `hiwavebrowser/hiwave-linux#86` (`castor/s2-sync-2026-10-29`, red `68200d3a`, fix `12464a45`, sync `d70b8fd2`), ready for R1 (Argos / Prometheus).
- **Blockers**: None.
- **Decisions for Atlas**:
  1. **Include the 3 new `S2` paint-order tests in `L1`**: `#824` (`positioned_paint_order_tests.rs`, 1 test) and `#826` (`flex_item_paint_order_tests.rs`, 2 tests) use the same `#[cfg(all(target_os = "macos", feature = "headless"))]` gate as the 9 tests in `L1`. Should I ungate all 12 (9 from `S1` + 3 from `S2`) in `L1`?
  2. **Queue the 3 `S2` red-test proposals on macOS `CA1`**: Should the 3 `S2` red-test proposals above (`FetchPolicy::reserve` budget leak, `DOMParser` frameset `body`, and `insertAdjacentHTML` on SVG `<title>`) be appended to `CA1` on the macOS Z plan?
