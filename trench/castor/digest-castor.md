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

## 2026-10-29 22:45 Castor L1

- **Before**: `hiwave-linux` `develop` at `f9e2f2ee` (with `#86` merged as `f7e41d5a`).
- **After**:
  - Deleted the accepted `S2` worktree (`/home/petec/repos/hiwave-linux-castor-s2` and its `target` directory) per fresh-checkout rule.
  - Created fresh worktree `/home/petec/repos/hiwave-linux-castor-l1` on branch `castor/l1-ungate-headless-tests`.
  - Executed `#86` follow-ups:
    1. Multi-threaded `console_log` receipt: `cargo test -p rustkit-engine console_log -- --test-threads=8` produced 36 passed, 0 failed.
    2. Verified `TEST_ALIASES` in `crates/rustkit-engine/src/console_log.rs` is enclosed under `#[cfg(test)]`.
  - Removed `#[cfg(all(target_os = "macos", feature = "headless"))]` gates from all 9 targeted tests and helper `image_rects`:
    1. `match_media_tests::match_media_answers_for_the_views_viewport` (before: gated out, after: PASS)
    2. `match_media_tests::match_media_follows_the_views_size` (before: gated out, after: PASS)
    3. `match_media_tests::a_resize_reports_change_to_the_lists_that_flipped` (before: gated out, after: PASS)
    4. `computed_content_tests::computed_content_is_the_cascaded_string_or_normal` (before: gated out, after: PASS)
    5. `abspos_percent_height_image_tests::a_percent_height_image_in_an_inset_sized_layer_fills_the_layer` (before: gated out, after: PASS)
    6. `abspos_percent_height_image_tests::an_abspos_image_is_placed_by_its_top_and_left` (before: gated out, after: PASS)
    7. `abspos_percent_height_image_tests::an_abspos_image_width_percent_resolves_against_the_inset_layer` (before: gated out, after: PASS)
    8. `abspos_percent_height_image_tests::a_static_image_is_unchanged` (before: gated out, after: PASS)
    9. `abspos_percent_height_image_tests::a_positioned_image_with_a_z_index_paints_over_a_later_sibling` (before: gated out, after: PASS)
  - **Workspace tests (`--test-threads=1`)**: **3,874 passed, 0 failed, 33 ignored** across workspace (`+9` passed over `develop`, 0 regressions).
  - **26-case Linux parity campaign**: **`1.1410%` mean** vs Mac `latest-develop.json` (`ff6ad36a`) **`1.1075%`** (delta **`+0.0335%`**, **`0 / 26`** cases over `+1.0 pt` vs Mac).
- **PRs**:
  - `hiwavebrowser/hiwave-linux#87` (`castor/l1-ungate-headless-tests`, initial `ca6a645d` -> delta `0eb73d2d`). Prometheus R1 CLEAR WITH NITS; Atlas promoted nit (gate on `feature = "headless"` rather than bare ungate to protect GPU-less CI runs). Head `0eb73d2d` carries `#[cfg(feature = "headless")]` on all 9 tests + helper. Workspace tests pass (3,874 passed, 0 failed, 33 ignored), GPU-less run verified (0 tests run, 0 adapter errors).
- **Blockers**: None. Ready for land.
- **Decisions for Atlas**:
  1. **Next row while waiting for #87 merge**: Row `L2` (Parity tooling parity: `scripts/parity_lib.py`, `cases/registry.json`, stale templates, test script paths) is `open`. Should I begin fresh checkout for `L2` immediately or wait for `#87` to land on `develop`?

## 2026-11-05 01:35 Castor L2

- **Before**: `hiwave-linux` `develop` at `21cb0309` (with `#87` merged as `8b4d7afc`).
- **After**:
  - Fresh worktree `/home/petec/repos/hiwave-linux-castor-l2` on branch `castor/l2-parity-tooling`.
  - Synced `scripts/parity_lib.py` with modern `--html-file`/`--dump-frame` flags, single source of truth from `cases/registry.json`, dynamic `CARGO_TARGET_DIR` binary discovery, and node path resolution supporting `~/.local/node22/bin`.
  - Synced `cases/registry.json` (26 non-holdout cases) and `cases/declaration_gaps.json`.
  - Synced `crates/hiwave-app/src/ui/{about,chrome,chrome_rustkit,settings,shelf}.html` canonical templates from macOS.
  - Synced case HTML files in `websuite/cases/`, `websuite/micro/`, `websuite/common/`.
  - Synced `scripts/parity_test.py` and modern `scripts/parity_swarm.py`.
  - Added existence guards for kept-deleted macOS paths in `scripts/tests/test_inspector_console_groups.py` (skips missing `inspector.html`), `scripts/tests/test_capture_suites_actually_run.py` (skips missing `parity.yml`), and `scripts/tests/test_js_suites_actually_run.py` (skips missing `parity.yml`).
  - Updated `.gitignore` to ignore parity capture/results output directories and `node_modules/`.
  - Configured `LimitNOFILE=524288:524288` in `null-daemon-castor.service`, verified daemon process has 524288 limit matching `ulimit -n 65536` in test runs.
  - **Workspace tests (`--workspace --no-fail-fast -- --test-threads=1`)**: **3,874 passed, 0 failed, 33 ignored** across 56 test suites.
  - **26-case Linux parity campaign**: **`1.1410%` mean** vs Mac `latest-develop.json` (`ff6ad36a`) **`1.1075%`** (delta **`+0.0335%`**, **`0 / 26`** cases over `+1.0 pt` vs Mac).
- **PRs**:
  - `hiwavebrowser/hiwave-linux#88` (`castor/l2-parity-tooling`, head `c8a46971`), ready for R1 (Argos / Prometheus).
- **Blockers**: None. Awaiting R1 review on `#88`.
- **Decisions for Atlas**:
  1. **Row L3 unblocking**: Next row is `L3` (App parity: 16 MiB main-thread stack linker flag in `crates/hiwave-app/build.rs`, X11 `WindowSurface`/`ContentThread` wiring, and restoring `ui/inspector.html`). Should I prepare a fresh worktree for `L3` while waiting for `#88` review, or wait for `#88` to land?

## 2026-11-05 04:45 Castor L3

- **Before**: `hiwave-linux` `develop` at `f86a93dc` (with `#88` merged as `79f6b621`).
- **After**:
  - Fresh worktree `/home/petec/repos/hiwave-linux-castor-l3` on branch `castor/l3-app-parity`.
  - Configured 16 MiB main-thread stack linker flag on Linux (`-Wl,-z,stack-size=16777216`) in `crates/hiwave-app/build.rs`.
  - Added red-first Linux test in `crates/hiwave-app/tests/main_thread_stack.rs` (asserts `readelf -lW` reports `GNU_STACK` `memsz` == `0x1000000`).
  - Restored canonical `crates/hiwave-app/src/ui/inspector.html` (~67 KB) from `hiwave-macos` and wired `pub const INSPECTOR_HTML: &str = include_str!("ui/inspector.html");` in `crates/hiwave-app/src/main.rs`. Verified `scripts/tests/test_inspector_console_groups.py` passes 4/4.
  - Implemented `WindowSurface::attach_x11(parent: u64, bounds: Bounds)` on Linux in `crates/rustkit-engine/src/presenter.rs` and exported `WindowSurface` unconditionally in `crates/rustkit-engine/src/lib.rs`.
  - Added `clip_raw_view` no-op fallback in `crates/rustkit-viewhost/src/lib.rs` for `not(target_os = "macos")`.
  - Ported `crates/hiwave-app/src/webview_rustkit.rs`, extracting X11 window handle from `tao` window and wiring `WindowSurface::attach_x11` + `ContentThread::spawn` on Linux when `content_thread_enabled()` is true.
  - Added optional dependency `hiwave-shield-intercept` to `crates/hiwave-app/Cargo.toml` and wired `shield_adapter` in `main.rs`.
  - Added test `window_surface_attaches_and_presents_on_x11` in `crates/rustkit-engine/src/linux_carried_tests.rs`.
  - Verified `hiwave` binary builds with default features and `--features rustkit`, and launches cleanly on X11 (`DISPLAY=:0 timeout 2s target/debug/hiwave`).
  - **Workspace tests (`--workspace --no-fail-fast -- --test-threads=1`)**: **3,876 passed, 0 failed, 33 ignored** across 57 non-empty suites (+2 passed over L2).
  - **26-case Linux parity campaign**: **`1.1410%` mean** vs Mac `latest-develop.json` (`ff6ad36a`) **`1.1075%`** (delta **`+0.0335%`**, **`0 / 26`** cases over `+1.0 pt` vs Mac).
- **PRs**:
  - `hiwavebrowser/hiwave-linux#89` (`castor/l3-app-parity`, initial `54366282` -> delta `df780a5f` -> head `cb06f5d5`). Prometheus R1 CLEAR WITH NITS. Fixed via `RLIMIT_STACK` raise to 16 MiB + re-exec in `crates/hiwave-app/src/main.rs`, added runtime probe `main_thread_stack_size()`, active 10 MiB stack recursion probe (`probe_10mib_stack_recursion`) with `black_box(&mut frame)`, updated `main_thread_stack.rs` test with runtime probe (asserts `sz >= 15 MiB`, verified `16769024` bytes, 10 MiB recursion ok without SIGSEGV), added `#[allow(dead_code)]` to `INSPECTOR_HTML`, cleaned up `HIWAVE_STACK_REEXEC` env var for child processes, and updated PR disclosures. Head `cb06f5d5` pushed; ready for Talos merge.
- **Blockers**: None. Awaiting Talos merge of `#89`.
- **Decisions for Atlas**:
  1. **Next package queueing**: With L1, L2, and L3 all completed (`#87` and `#88` landed, `#89` in review), all packages currently listed in `PLAN-castor.md` are executed. What package should be queued next for Castor (e.g., next engine sync `3c6b170e..HEAD` or next tranche of Linux platform polish)?

## 2026-11-05 09:55 Castor standing by

- **Before**: `hiwave-linux` `develop` at `f86a93dc` / `e96ee219`, PR `#89` in review at head `cb06f5d5`.
- **After**:
  - PR `#89` (Package `L3`: 16 MiB main-thread stack, X11 surface wiring, `inspector.html` restore) approved (Prometheus R1 CLEAR WITH NITS) and LANDED on `develop` as merge commit `aa0fbdda` by Talos.
  - Worktree `/home/petec/repos/hiwave-linux-castor-l3` deleted per fleet rules after land; `develop` branch synced to `aa0fbdda`.
  - Packages S1, A1, S2, L1, L2, L3 are all complete and landed on `develop`.
  - Standing by for Atlas's next package row in `PLAN-castor.md` (e.g. S3 sync `366fdd48..e273c0bf` / Batch 19b).
- **PRs**: None open. `#89` merged as `aa0fbdda`.
- **Blockers**: None. Awaiting next package row in `PLAN-castor.md`.
- **Decisions for Atlas**:
  1. **Next package row**: All rows in `PLAN-castor.md` (S1, A1, S2, L1, L2, L3) are landed and marked `done`. Standing by for next orders (e.g. S3 sync).

## 2026-11-06 10:55 Castor standing by for S3 R1 review

- **Before**: `hiwave-linux` `develop` at `588742df`, all prior packages (`S1`, `A1`, `S2`, `L1`, `L2`, `L3`) landed.
- **After**:
  - Received Pete-direct assignment via Talos broadcast: Talos authors package `S3` (syncing `hiwave-linux` `develop` to `hiwave-macos` `develop` `95691f85` / Batch 29, 3-way from `3c6b170e`).
  - Pete designated Castor as the R1 reviewer of record for Talos's S3 PR; Atlas confirmed Castor's R1 at exact head SHA is the review of record before Atlas merges.
  - Broadcast `#19` posted to exchange acknowledging assignment.
  - Preparing conflict review checklist across the 19 shared files and 8 conflict paths (notably `hiwave-app/src/main.rs` preserving L3 16 MiB stack, X11 wiring, and Linux webview-fallback default).
- **PRs**: None open from Castor. Awaiting Talos's `talos/s3-sync-2026-11-06` PR.
- **Blockers**: None.
- **Decisions for Atlas**: None. Standing by for Talos's PR head to perform R1 review.

## 2026-11-06 13:00 Castor R1 on PR #90 (S3 Sync)

- **PR**: `hiwavebrowser/hiwave-linux#90` (`talos/s3-sync-2026-11-06`, author: Talos).
- **Head SHA**: `55d9da9a0dc6d43e19388e38f95d2a89c3aba6af` (short `55d9da9a`). Base: `develop`.
- **Verdict**: **R1 CLEAR WITH NITS** posted on GitHub PR #90 and broadcast #20 to exchange.
- **Audit Findings**:
  - `crates/hiwave-app/src/main.rs`: Retains Linux `webview-fallback` default and builds all feature combinations (`webview-fallback`, `rustkit`, `native-linux`). L3 work (16 MiB stack raise, runtime probe, `main_thread_stack.rs`, and X11 surface wiring) verified 100% intact and passing.
  - Shared Engine Code: `console_log.rs` taken verbatim from macOS; `rustkit-engine/src/lib.rs` 3 hunks cleanly resolved; carried flex tests pass; single-threaded `console_log::tests` passes 37/37.
  - Deletions: All 8 macOS-only files verified absent; `css_census` dropped.
  - Privacy Logging: Ported helpers from #946 correctly wrap 11 URL and 3 IPC logging sites; `url_log_tests` pass 3/3.
  - Evidence: `cargo check --workspace --tests` clean (0 errors); `main_thread_stack.rs` 1/1 pass; `linux_carried_tests` 157/157 pass; campaign mean 1.1245% vs Linux Chrome 148 baselines (0/26 over +1.0 pt).
- **Decisions for Atlas**:
  1. **Option A for LAT2 test**: Concur with Talos on declaring divergence via `#[cfg_attr(target_os = "linux", ignore = "...")]` and queuing a dedicated Linux glyph cache latency row as the next Linux package.
  2. **Merge Go**: PR #90 is CLEAR to land on `develop` once Atlas approves.

## 2026-11-06 14:00 Castor L4 (LAT2 Spinner Test Ignore)

- **PR**: `hiwavebrowser/hiwave-linux#91` (`castor/l4-lat2-ignore`).
- **Head SHA**: `a6e355bc` (ready for review against `develop`). Red commit: `b39da378`.
- **Scope**: Declares known Linux performance divergence on `paint_only_animation_tests::a_spinner_on_a_500_box_page_never_relays_out_and_is_cheap` via `#[cfg_attr(target_os = "linux", ignore = "...")]`, referencing hub row `LATLX` (Option A adopted by Atlas). Touches only `crates/rustkit-engine/src/paint_only_animation_tests.rs` (+4 lines).
- **Evidence**:
  - Red run failed on Linux: painted frame 172.70 ms vs relayout 256.12 ms (`painted_ms * 5.0 <= full` assertion failed).
  - Green run with ignore attribute: `1 ignored; 0 failed`.
  - Workspace test run: `integration_tests` 27/27 pass, `main_thread_stack` 1/1 pass, budget tests pass, 0 unexpected failures across workspace. PR #91 marked ready.
- **Sequencing**:
  - Confirmed to Atlas and Talos (broadcast #21) that L4 and upcoming L5 touch only `rustkit-engine` and `rustkit-text`, neither touching `main.rs` nor `viewhost/src/lib.rs`. Zero file conflicts with Talos's X11 RustKit chrome staging (S1/S2).
- **Decisions for Atlas**: None. Standing by for Argos R1 review.

## 2026-11-06 15:20 Castor R1 on PR #92 (Linux RustKit Chrome Staged Plan)

- **PR**: `hiwavebrowser/hiwave-linux#92` (`talos/chrome-plan-linux`, author: Talos).
- **Head SHA**: `45d2621ab0e52fdc09112b1c129b9e43ab6b39ac` (short `45d2621a`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #23 to exchange.
- **Audit Findings**:
  - Scope: Docs-only addition of `trench/chrome/PLAN-linux-chrome.md`.
  - Accuracy: Verbatim records the S0 finding (`ContentWebView::RustKit` currently unwired in `main.rs`, Linux `rustkit` feature doing nothing visible today).
  - Structure: Clearly defines Option H (macOS-shaped hybrid), XShape binary composition, known alpha gap (masked in S4), fallback rules, and isolated Xvfb (`:99`) / scratch profile constraints.
  - S1 Refinement: Approves splitting S1 into S1a (`ContentWebView::RustKit` behind `HIWAVE_RUSTKIT_CONTENT=1`), S1c (X11 input translation to `InputQueue` to resolve viewhost event discard), and S1b (`ChromeUi` port on WebKit).
  - Answered Talos regarding L3: L3 scoped only stack raise, surface wiring, and inspector restore; X11 input was intentionally deferred.
- **Decisions for Atlas**: PR #92 is CLEAR to land on `develop` (LANDED at 24adad78).

## 2026-11-06 15:55 Castor R1 on PR #93 (Drop GTK Edit Menubar & NVIDIA DMA-BUF Workaround)

- **PR**: `hiwavebrowser/hiwave-linux#93` (`talos/linux-window-fixes`, author: Talos).
- **Head SHA**: `42a64199d7e37fa9f61a2a69007fec95cfd96b27` (short `42a64199`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #24 to exchange.
- **Audit Findings**:
  - `platform/linux.rs`: Ceases attaching stray GTK Edit menubar (`init_for_gtk_window`), revealing the full tab strip (logo, active tab, close/new-tab controls).
  - `main.rs`: `apply_gpu_workarounds()` detects NVIDIA proprietary drivers (`/proc/driver/nvidia`, `/dev/nvidiactl`) and sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` before GTK/WebKit init (with user override preserved), resolving empty/blank window rendering under Wayland/XWayland. Correctly preserves logging across the 16 MiB stack re-exec.
  - Evidence: Verified in clean review worktree: `cargo check -p hiwave-app` clean on default and rustkit feature targets; `hiwave-app` `integration_tests` 27/27 pass; `main_thread_stack` 1/1 pass.
- **Seat Role Transition**: Per Pete's direct instruction, Castor operates in **reviews-only mode** from this point forward (no code authoring or new branches). Open code PR #91 is left for Argos R1 and Atlas to merge or close; L5 (Linux glyph cache) transferred to Talos.
- **Decisions for Atlas**: PR #93 is CLEAR to merge on `develop` (LANDED at 5234107c).

## 2026-11-06 16:25 Castor R1 on PR #94 (GNOME Title Bar Layout Offset Fix)

- **PR**: `hiwavebrowser/hiwave-linux#94` (`talos/linux-window-layout`, author: Talos).
- **Head SHA**: `b314430f415b1e34e31065a0fe21560fdbd82375` (short `b314430f`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #25 to exchange.
- **Audit Findings**:
  - `main.rs`: Fixes window layout offset on GNOME Wayland desktop where GTK places the container 47 px below its decorated title bar, while `wry::set_bounds` sizes views relative to the window origin.
  - Dynamically captures container position via `fixed.connect_size_allocate` into `LAYOUT_ORIGIN_X`/`LAYOUT_ORIGIN_Y`, firing `UserEvent::Relayout` on container movement.
  - Offsets `chrome_rect`, `content_rect`, and `shelf_rect` in `apply_layout` while reducing usable height, ensuring the tab strip remains visible under decorations (and evaluates to 0 on undecorated X11).
  - Evidence: Verified in clean review worktree: `cargo check -p hiwave-app` clean; `hiwave-app` `integration_tests` 27/27 pass; `main_thread_stack` 1/1 pass.
- **Decisions for Atlas**: PR #94 is CLEAR to merge on `develop` (LANDED at c915206c).

## 2026-11-06 17:55 Castor R1 on PR #95 (Stage S1a + S1c: RustKit Content View & X11 Input)

- **PR**: `hiwavebrowser/hiwave-linux#95` (`talos/chrome-s1a`, author: Talos).
- **Head SHA**: `4b3b887ea67744d90a9543bde420450140f07d96` (short `4b3b887e`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #28 to exchange.
- **Audit Findings**:
  - `Cargo.toml`: Reverses default feature to `default = ["webview-fallback", "rustkit"]`, enabling both engines to be compiled simultaneously for runtime selection (aligning with macOS pattern).
  - `content_webview_enum.rs`: Unifies `ContentWebView` as an enum supporting `Wry` and `RustKit`, implementing `IWebView` and `ContentWebViewOps`.
  - `rustkit_content.rs`: Manages `HIWAVE_RUSTKIT_CONTENT=1` runtime flag:
    - Default-off: proven 0 cost with flag unset (no child window, no thread, no view created).
    - Wayland fallback: clear diagnostic log line explaining requirement for `GDK_BACKEND=x11` and safe fallback to WebKit.
    - Engagement on X11: attaches child view, starts 16 ms pump routing events/IPC/navigations/state into main event loop, timestamped startup logs.
    - Active load stepping: steps active document loads during pump so navigation does not hang.
  - `crates/rustkit-viewhost/src/x11_input.rs` (Stage S1c):
    - Translates X11 mouse button press/release/motion/enter/leave.
    - Translates mouse scroll buttons (4-7) mapped to 40px offsets.
    - Keysym translation to Win32 VK codes for parity with Windows/macOS event loop expectations.
    - Translates events to `InputQueue` for delivery to content thread.
  - Bug fixes:
    - `focus_view` recursion in `crates/rustkit-viewhost/src/lib.rs:1603` fixed (replaces self-call with platform call).
  - `paint_only_animation_tests.rs`: Folds in the 4-line LAT2 spinner test ignore attribute from #91; PR #91 closed.
  - Evidence: Verified in clean review worktree at exact head `4b3b887e`:
    - `cargo check -p hiwave-app`: clean (0 errors).
    - `cargo test -p rustkit-viewhost x11_input`: 11/11 passed.
    - `cargo test -p hiwave-app --bin hiwave rustkit_content`: 5/5 passed.
    - `cargo test -p hiwave-app --test main_thread_stack`: 1/1 passed.
    - `cargo test -p hiwave-app --test integration_tests -- --test-threads=1`: 27/27 passed.
    - Folded LAT2 spinner test: 1 ignored / 0 failed (`LATLX`).
- **Decisions for Atlas**: PR #95 is CLEAR to merge on `develop` (LANDED at `ec01c8ce` by Pete).

## 2026-11-06 23:00 Castor R1 on PR #96 (Stage S1b: ChromeUi / ChromeView / LazyPanel)

- **PR**: `hiwavebrowser/hiwave-linux#96` (`talos/chrome-s1b`, author: Talos).
- **Head SHA**: `b27a356f55bdbcb9fdfe93d2a4aeea456149feb1` (short `b27a356f`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #31 to exchange.
- **Audit Findings**:
  - `crates/hiwave-app/src/chrome_ui.rs`:
    - Ported `ChromeUi` trait, matching macOS `main.rs:327` trait definition byte-for-byte in semantics and default implementations (`eval`, `place`, `take_keyboard`, `compose`, `keep_on_top`).
    - `impl ChromeUi for wry::WebView` delegates `eval` to `evaluate_script` and `place` to `set_bounds`.
    - `ChromeView::WebKit` wraps `Arc<wry::WebView>` and implements `IWebView`, delegating all 10 methods without altering behavior.
    - `LazyPanel`: Implements bounded script retention (`LAZY_PANEL_HELD_MAX = 256`) and deferred creation on open.
  - `crates/hiwave-app/src/main.rs`:
    - Holds `chrome_webview` and `shelf_webview` as `ChromeView`.
    - All call sites retain identical semantics (`shelf_for_events.as_ref()` replacing `as_deref()`).
  - Evidence: Verified in clean review worktree at exact head `b27a356f`:
    - All 4 feature builds compile cleanly: default (`webview-fallback` + `rustkit`), `native-linux`, `webview-fallback` only, and `rustkit` only.
    - `cargo test -p hiwave-app --bin hiwave chrome_ui`: 5/5 passed.
    - `cargo test -p hiwave-app --bin hiwave`: 71/71 passed.
    - `cargo test -p hiwave-app --test integration_tests -- --test-threads=1`: 27/27 passed.
    - `cargo test -p hiwave-app --test main_thread_stack`: 1/1 passed.
    - Live smoke launch on X11 (`DISPLAY=:0 timeout 2s target/debug/hiwave`): clean startup, 15 MiB stack, DMA-BUF workaround active, IPC and tab sync verified.
- **Decisions for Atlas**: PR #96 is CLEAR to merge on `develop`.
## 2026-11-07 00:08 Castor R1 on PR #97 (Stage S2: RustKit Chrome behind HIWAVE_RUSTKIT_CHROME=1)

- **PR**: `hiwavebrowser/hiwave-linux#97` (`talos/chrome-s2`, author: Talos).
- **Head SHA**: `b68be04470c6a797e77535776feb0154033fe8a5` (delta `c7805706..b68be044`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #33 to exchange.
- **Audit Findings**:
  - **(1) Default-off**: `decide_chrome` returns `Off` unless `HIWAVE_RUSTKIT_CHROME=1`. Emits single `debug!` line. All 7 unit tests pass.
  - **(2) Fallback lines**: Wayland and missing content thread paths emit single `warn!` lines naming remediation and fall back cleanly to WebKit chrome without panic.
  - **(3) libXext FFI & XShape Composition**: Local declarations match `<X11/extensions/shape.h>`. `XRectangle` values mapped explicitly from tuples into `Vec<XRectangle>` (`len = n_rects`, offset `(0, 0)` in chrome window coordinates). Normal layout applies `compose_region` with `Some(&region)` (window less page hole).
  - **(4) Coordinate Clamping**: Clamping outward to `i16::MIN..i16::MAX`. Straddling test `a_rectangle_straddling_the_i16_limit_is_cut_at_it` added and passed. All 45 `rustkit-viewhost` unit tests pass.
  - **(5) Chord Key Text Insertion Safety**: Audited keydown-to-text-insertion path. Shortcut chords (`primary || alt`) return `Unhandled` in `forms.rs:1968` and `!ctrl` guard in `selection.rs:582` prevents any insertion into focused text controls or contenteditable elements. Pinned by new unit test `a_chord_with_its_key_character_inserts_nothing_into_a_focused_text_control` in `rustkit-engine/src/linux_carried_tests.rs`. Passed.
  - **(6) Feature Builds**: Default, `native-linux` (clean check with `std::sync` properly gated), `webview-fallback`, and `rustkit` all compile cleanly.
  - **(7) Test Verification**: 79/79 `hiwave-app` unit tests pass, 27/27 integration tests pass, 1 stack test passes; 45/45 `rustkit-viewhost` tests pass; 870 `rustkit-engine` lib tests pass.
  - **(8) Documentation**: `README.md` updated with `libxext-dev` (Debian/Ubuntu), `libXext-devel` (Fedora), and `libxext` (Arch Linux) plus explanatory note.
- **Decisions for Atlas**: PR #97 is CLEAR to merge to `develop`. Stage S3 (overlay growth rules, chrome creation-failure fallback, e.key field) opens next.

## 2026-11-07 01:28 Castor R1 on PR #98 (Stage S3 Part 1: Focus Return & Fallback)

- **PR**: `hiwavebrowser/hiwave-linux#98` (`talos/chrome-s3`, author: Talos).
- **Head SHA**: `849acd6cae32dc7ab0ab243a2f49504c73f7e035` (develop merged in). Base: `develop`.
- **Verdict**: **R1 HOLD** posted on GitHub and broadcast #35 to exchange.
- **Audit Findings**:
  - **(1) HOLD: Focus Return (`rustkit_chrome::return_focus_on_page_click`)**:
    - Closure passes timestamp 0 (`GDK_CURRENT_TIME`) and calls `gdk_window.focus`.
    - In standard desktop environments with a window manager (_NET_ACTIVE_WINDOW), `gdk_x11_window_focus` sends a client message with timestamp 0, which focus-stealing prevention drops.
    - Chrome take-focus call is `XSetInputFocus(self.display, window, RevertToParent, CurrentTime)` in `rustkit-viewhost/src/linux.rs:272` (via `webview_rustkit.rs:773` -> `presenter.rs:389`).
    - Fix: Invert the take call via raw `XSetInputFocus(toplevel_xid, RevertToParent, CurrentTime)` inside GDK error trap, or pass `ev.time()` and prove `_NET_ACTIVE_WINDOW` arm under openbox in Xvfb.
  - **(2) CLEAR: Creation Order & Stacking**:
    - `rustkit_chrome_view` created at `main.rs:1476` before page (`main.rs:1700-1743`).
    - When `view.raise()` runs at line 2159, page X window already exists, ensuring chrome raises above it.
  - **(3) CLEAR: Creation-Failure Fallback**:
    - `rustkit_chrome_on` set to `rustkit_chrome_view.is_some()`; failure cleanly falls back to WebKit chrome at line 1503. Zero subsequent reads re-query the env flag.
    - `create()` does not call `XShapeQueryExtension` (noted for S3 part 2).
  - **(4) CLEAR: Tests & CI**:
    - 79/79 `hiwave-app` tests pass.
    - GHA Run 42345068474: Dependency install green; reachability ratchet is the sole failure (standing exception since #85).
- **Decisions for Atlas**: PR #98 on HOLD pending Talos delta on focus return (timestamp / XSetInputFocus inversion).

## 2026-11-07 01:56 Castor R1 Delta on PR #98 (Stage S3 Part 1: Focus Return & Fallback)

- **PR**: `hiwavebrowser/hiwave-linux#98` (`talos/chrome-s3`, author: Talos).
- **Head SHA**: `af8cf28d36f33b805c19e5cbe876d818b84dc760` (delta `849acd6c..af8cf28d`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #36 to exchange.
- **Audit Findings**:
  - **(1) Focus Return on Page Click**: Inverted take call via `chrome.focus_x11_window(toplevel)` calling `X11ViewHost::focus_window(window)` (`XSetInputFocus(self.display, window, RevertToParent, CurrentTime)`). Direct raw X call bypassing WM mediation entirely. Verified under mutter + XWayland (`gate-runner/wl_x11_focus.sh`, `_logs/wlx/b_typed.png`, `c_after_page_ctrl_t.png`).
  - **(2) XShape Extension Probe**: `create()` starts with `x11_shape_supported()` via `XShapeQueryExtension`. Dedicated short-lived connection cleanly closed.
  - **(3) Atlas Checks (c1 - c4)**:
    - `(c1)`: Mutter run drove WebKit page (`UnifiedContentWebView::Wry`), the default page path. Chrome + RustKit page combo tracked for S3 part 2.
    - `(c2)`: Chrome takes `surface` arm in `with_x11_view` (`HIWAVE_CONTENT_THREAD=1`); borrow is brief, no contention during GTK dispatch.
    - `(c3)`: Silent None returns on `toplevel_xid` / missing X11 host tracked for S3 part 2 to add `warn!`.
    - `(c4)`: `PageEndpoint::focus_x11_window` cfg guard tracked for S3 part 2.
  - **(4) Tests & CI**: 79/79 `hiwave-app` tests pass; 45/45 `rustkit-viewhost` tests pass. GHA Run 42347619838 build step green, reachability ratchet is the sole failure (standing exception since #85).
- **Decisions for Atlas**: PR #98 is CLEAR to merge to `develop`. Ready for live Atlas / Pete merge.

## 2026-11-07 02:59 Castor R1 on PR #99 (Stage S3 Part 2a: Cursor Shape & Shifted Chord Keys)

- **PR**: `hiwavebrowser/hiwave-linux#99` (`talos/chrome-s3b`, author: Talos).
- **Head SHA**: `e662aa27c70e20c8f9b0c67b9d7b938070612eb4` (develop merged in). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #37 to exchange.
- **Audit Findings**:
  - **(1) Cursorfont Glyph Numbers & Lifecycle**: All 16 glyph numbers in `x11_cursor_shape` match `<X11/cursorfont.h>` (even, < 154). `XCreateFontCursor` -> `XDefineCursor` -> `XFreeCursor` -> `XFlush` lifecycle conforms to Xlib spec (server keeps window cursor attribute reference; client handle freed cleanly). Dispatched on GTK main thread from `PageEvent::Cursor`. XFixes probe artifact confirmed in PR body (`xterm`, `hand2`, `left_ptr`).
  - **(2) Shifted Chord Keys (`chord_key`)**:
    - Digits indexed into `b")!@#$%^&*("` (0x30..=0x39).
    - 9 OEM arms paired correctly for US layout.
    - `0xBB` (`=` / `+`) and `0xBD` (`-` / `_`) verified present at lines 2752 & 2754 in `webview_rustkit.rs` (zoom shortcuts fully supported).
  - **(3) Atlas Checks (c2, c3, Warnings)**:
    - `(c2)`: Chrome takes `surface` arm in `with_x11_view` (`HIWAVE_CONTENT_THREAD=1`); dispatched on GTK main thread without borrow contention.
    - `(c3)`: "auto"/"default" maps to `XC_left_ptr` (68), substituted with active theme cursor under desktop libXcursor theming.
    - Warnings on silent paths: `rustkit_chrome::return_focus_on_page_click` warns if `toplevel_xid` is None; `ViewHost::focus_x11_window` warns if no X11 host.
  - **(4) Tests & CI**: 79/79 `hiwave-app` tests pass; 46/46 `rustkit-viewhost` tests pass (added `css_cursors_map_to_x11_font_cursors`). All 4 feature builds pass. GHA Run 42351544264 build step green, reachability ratchet is the sole failure (standing exception since #85).
- **Decisions for Atlas**: PR #99 is CLEAR to merge to `develop`. Ready for live Atlas / Pete merge.


## 2026-11-07 04:55 Castor R1 on PR #100 (Stage L5: LATLX Text Shape Cache)

- **PR**: `hiwavebrowser/hiwave-linux#100` (`talos/l5-shape-cache`, author: Talos).
- **Head SHA**: `b2e922cbabd9a212ba38badb14f19ce131c5e831` (off develop). Base: `develop`.
- **Verdict**: **R1 HOLD** posted on GitHub and broadcast #38 to exchange.
- **Audit Findings**:
  - **(1) Parity Framing**: Confirmed macOS `crates/rustkit-layout/src/text.rs:1500-1546` implements the identical per-thread shape memo (`MAX_ENTRIES = 16384`, drop-whole, clone on hit, no borrow across uncached call). #100 is reference parity rather than a Linux-only divergence. PR description owed an update to cite reference parity.
  - **(2) Substance s1 (HOLD)**:
    - macOS memo architecture stores `generation: u64` inside `struct Memo { generation: u64, runs: HashMap<Key, ShapedRun> }` and clears runs on entry if `m.generation != generation`.
    - `Key` in #100 includes `generation` as a tuple field and never clears when `generation` increments, retaining dead entries until the 16384 cap.
    - HOLD for Talos to align with the macOS structure: `generation` out of `Key`, and compare-and-clear in `MEMO.with`.
  - **(3) Substance s2 (HOLD)**:
    - FreeType investigation: `LinuxTextBackend::get_face` calls `face.set_char_size((size * 64.0) as isize, ...)` which fails in FreeType with `Invalid_Pixel_Size` for `size <= 0.0`.
    - This causes `shape_uncached` to fall back to `shape_stub`, allocating non-empty glyph vectors and caching them in the memo.
    - HOLD for Talos to add the macOS early guard `if size <= 0.0 { ... }` before the memo lookup, returning an empty run with 0.0 metrics.
  - **(4) Code Audits**:
    - `(k1)`: `LinuxTextBackend::resolve_family` is not site-scoped. Webfonts are partitioned per `TopLevelSite` during download/parsing in `FontFaceLoader`, but registered in `rustkit_text::webfonts` with process-level `generation` bump. Shape memo key on macOS and Linux both omit site (exact parity).
    - `(k2)`: Confirmed no transient Err arms in `advance_widths`. All error paths are deterministic (missing font in fontconfig, FreeType face construction failure, FreeType char load failure). Caching `shape_stub` on failure is deterministic and consistent.
    - Key coverage: `(text, families, weight, style, stretch, size)` covers all inputs of `shape_uncached`.
  - **(5) Tests & CI**:
    - LAT2 spinner test `a_spinner_on_a_500_box_page_never_relays_out_and_is_cheap` unignored and passes in 3.60s (closes row L4/LATLX).
    - `the_linux_shape_cache_agrees_with_the_uncached_measure_and_separates_its_inputs` passes.
    - `rustkit-layout` unit tests: 875 passed, 0 failed, 16 ignored.
    - Allowlisted engine failures: Menlo caret (`text_control_select_tests::a_click_places_the_caret_at_the_click_point`) and plain-inline transform (`transform_rect_tests::a_transform_does_not_apply_to_a_plain_inline`) match `linux_known_fail.txt`.
    - CI: Linux Metrics (Run 42359859584) BUILD step is green; reachability ratchet is the standing documented exception.
- **Decisions for Atlas**: PR #100 on HOLD pending Talos delta addressing (s1) and (s2) plus PR description update.

## 2026-11-07 05:25 Castor R1 Delta on PR #100 (Stage L5: LATLX Text Shape Cache)

- **PR**: `hiwavebrowser/hiwave-linux#100` (`talos/l5-shape-cache`, author: Talos).
- **Head SHA**: `29b18edfcf7badc527bae434b32d867a9aa5bbe9` (delta `b2e922cb..29b18edf`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #39 to exchange.
- **Audit Findings**:
  - **(1) Parity Framing**: PR description updated by Talos to cite reference parity with macOS `crates/rustkit-layout/src/text.rs:1500-1546`.
  - **(2) Substance s1 (CLOSED)**:
    - `struct Memo { generation: u64, runs: HashMap<Key, ShapedRun> }` implemented in thread-local storage.
    - Compares `m.generation != generation` on entry and clears cached runs before inserting.
    - `Key` updated to `(String, String, Vec<String>, u16, u8, u8, u32)` matching macOS reference; `generation` removed from the key.
  - **(3) Substance s2 (CLOSED)**:
    - Early guard `if size <= 0.0 { ... }` added before memo lookup, returning empty run with `TextMetrics::with_font_size(0.0)`.
    - Prevents FreeType `Invalid_Pixel_Size` and eliminates dummy glyph stub allocations.
  - **(4) Tests & CI**:
    - `rustkit-layout` unit suite: 877 passed, 0 failed, 16 ignored. All 3 new tests (`font_size_zero_shapes_to_nothing`, `a_new_font_generation_empties_the_memo`, `the_linux_shape_memo_agrees_with_the_uncached_measure_and_separates_its_inputs`) pass cleanly without parallel race flakes.
    - LAT2 spinner test `a_spinner_on_a_500_box_page_never_relays_out_and_is_cheap` unignored and passes in 3.41s, closing row L4/LATLX.
    - GHA Run 42362145940 build step is green; reachability ratchet is the standing documented exception.
- **Decisions for Atlas**: PR #100 is CLEAR to merge to `develop` at frozen head `29b18edfcf7badc527bae434b32d867a9aa5bbe9`.

## 2026-11-07 06:40 Castor R1 on PR #101 (Stage S4: hiwave-macos Sync 95691f85..5b4371ec)

- **PR**: `hiwavebrowser/hiwave-linux#101` (`talos/s4-sync-2026-11-07`, author: Talos).
- **Head SHA**: `241e3d6d0008dc7793ff6b60c0812f8a62ede09c` (develop merged in). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #40 to exchange.
- **Audit Findings**:
  - **(1) Feature Builds**: All 4 `hiwave-app` feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
  - **(2) Test Counts (Independently Verified)**:
    - `rustkit-layout`: 886 passed, 0 failed, 16 ignored.
    - `rustkit-bindings`: 481 passed.
    - `rustkit-css`: 153 passed.
    - `rustkit-text`: 85 passed (79 lib + 6 integration).
    - `rustkit-dom`: 109 passed.
    - `rustkit-renderer`: 144 passed.
    - `rustkit-viewhost`: 46 passed.
    - `hiwave-app`: 107 passed (79 unit + 27 integration single-threaded + 1 stack).
    - `rustkit-engine --lib` (single-threaded): 899 passed, 0 failed, 6 ignored.
    - `rustkit-engine --features headless --lib`: 2010 passed, 2 allowlisted failures (`text_control_select_tests::a_click_places_the_caret_at_the_click_point` and `transform_rect_tests::a_transform_does_not_apply_to_a_plain_inline` matching `linux_known_fail.txt`), 33 ignored.
    - LAT2 spinner test `a_spinner_on_a_500_box_page_never_relays_out_and_is_cheap` passes in 3.32s; L5 memo active.
  - **(3) Conflict Resolutions & Code**:
    - `hiwave-app/src/main.rs`: Retained Linux structure cleanly.
    - `chrome_state.rs` & `inspector_page_tests.rs`: Un-`mod`'d files cause zero compile exposure; tracked for follow-up PRs.
    - `chrome.html` & `inspector.html`: Contract checked against Linux `main.rs`. Methods called directly via `evaluate_script` remain fully functional and gain quote escaping and focus guard improvements.
    - `scripts/realsite_board.py` & `tools/parity_oracle/realsite.mjs`: Honest user-agent tooling preserved without fake UA strings.
    - `crates/rustkit-engine/src/lib.rs`: X11 `create_view` arm initializes `focus_by_pointer: false, space_armed: None`, matching macOS.
  - **(4) Tests & CI**: GHA Run 42366698620 BUILD step is green; reachability ratchet is the standing documented exception.
- **Decisions for Atlas**: PR #101 is CLEAR to merge to `develop` at frozen head `241e3d6d0008dc7793ff6b60c0812f8a62ede09c`, subject to Talos enumerating the 9 conflict paths in the body per Atlas broadcast #1178.

## 2026-11-07 07:25 Castor R1 on PR #102 (Stage L5 Test Robustness)

- **PR**: `hiwavebrowser/hiwave-linux#102` (`talos/l5-test-robustness`, author: Talos).
- **Head SHA**: `2b6aa30ec9f9d8c56b5622fdab10ab6cbab6c2c7` (base: `develop` at `be660aa7`).
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #41 to exchange.
- **Audit Findings**:
  - **(1) Atlas HOLD Lifted**: Atlas confirmed lift of initial HOLD in broadcast #1180 following fix f1 push.
  - **(2) Fix f1 Audit**:
    - `crates/rustkit-layout/src/text.rs`: `within_one_generation` signature updated to `fn within_one_generation(mut body: impl FnMut(usize) -> Result<(), String>) -> usize`.
    - Discarded retry attempts no longer contaminate thread-local `SHAPE_MEMO`: test strings in `the_linux_shape_memo_agrees_with_the_uncached_measure_and_separates_its_inputs` and `a_new_font_generation_empties_the_memo` now incorporate `{attempt}` index (`Handgloves quick wizard memo {attempt}`, `Handgloves {attempt}`, `generation probe text {attempt}`).
    - When an attempt is discarded due to concurrent web-font registration, the next attempt operates in a guaranteed clean key space without requiring unsafe clearing of the memo.
  - **(3) Tests & Invariant Verification**:
    - `rustkit-layout`: 886 passed, 0 failed, 16 ignored.
    - Memo tests pass cleanly and deterministically.
- **Decisions for Atlas**: PR #102 is CLEAR to merge to `develop` at head `2b6aa30ec9f9d8c56b5622fdab10ab6cbab6c2c7`. Ready for Talos / Atlas merge.

## 2026-11-07 07:50 Castor PR #102 Landed & Standing By

- **develop**: Advanced to `ffa4bc7f` with PR #102 merge (`Merge pull request #102 from hiwavebrowser/talos/l5-test-robustness`).
- **Fleet Updates**:
  - Talos broadcast #404/#405: 26-case Linux campaign on merged develop re-verified at 1.1245% mean (0 errors, 26/26 scored).
  - Atlas broadcast #1181/#1182: PR #102 merged; CH3.A10 chrome-state coalescing deferred behind synthetic load experiment / hand-test trigger.
  - Active Talos queue: release frame-time run, then S3 part 2b (or Batch 34 sync).
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's next PR for R1 review.

## 2026-11-07 10:10 Castor R1 on PR #103 (Stage S5: hiwave-macos Sync 5b4371ec..3b4561e3)

- **PR**: `hiwavebrowser/hiwave-linux#103` (`talos/s5-sync-2026-11-07`, author: Talos).
- **Head SHA**: `4194b4c4cbe25836fda72c6d657e8897ff1611ec` (develop merged in). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #43 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 15 files, +1265/-52 across `rustkit-bindings`, `rustkit-css`, `rustkit-cssparser`, `rustkit-engine`, `rustkit-layout`, `tools/parity_oracle`. Brings in Batch 34 from macOS develop `3b4561e3`:
    - #971: `<geometry-box>` keyword support on `clip-path` and `mask-origin`/`mask-clip` with proper SVG-to-CSS layout box fallbacks (`fill-box` to content box, `stroke-box`/`view-box` to border box).
    - #972: CSS declaration list splitting only at top-level `;` in `split_declaration_list` (Rust) and `splitDecls` (JS), preserving data URIs with `;base64,` and rejecting bad unquoted URLs per CSS Syntax 3 §4.3.6.
    - #973: Empty (`;;`) and colonless declarations skipped without dropping subsequent valid declarations per CSS Syntax 3 §5.4.5.
  - **(2) Feature Builds**: All 4 `hiwave-app` feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
  - **(3) Test Counts (Independently Verified)**:
    - `rustkit-cssparser`: 31 passed, 0 failed.
    - `rustkit-css`: 154 passed, 0 failed.
    - `rustkit-bindings`: 483 passed, 0 failed (482 lib + 1 census).
    - `rustkit-layout --lib`: 886 passed, 0 failed, 16 ignored.
    - `rustkit-engine --lib` (single-threaded): 902 passed, 0 failed, 6 ignored (+3 new tests: `empty_declaration_tests`, `style_attr_declaration_tests`).
  - **(4) Linux Stability**: Zero Linux platform code touched. `chrome_state.rs` remains un-`mod`'d and untouched, ready for CH3.A10 when scheduled. GHA Run 42381892494 BUILD step clean (reachability ratchet standing exception).
- **Decisions for Atlas**: PR #103 is CLEAR to merge to `develop` at head `4194b4c4cbe25836fda72c6d657e8897ff1611ec`.

## 2026-11-07 11:00 Castor R1 on PR #104 (Stage I1a: Linux Inspector Panel Shell)

- **PR**: `hiwavebrowser/hiwave-linux#104` (`talos/i1a-inspector`, author: Talos).
- **Head SHA**: `95030257b4f9b5cdc9f8fa11358747be72ab8cd5` (off `develop` at `bb0ae128`). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #44 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 5 files, +386/-3 across `inspector_panel.rs`, `ipc/commands.rs`, `ipc/mod.rs`, `main.rs`, and `rustkit-viewhost/src/lib.rs`. Implements the shell of the Linux Inspector panel (layout, view creation, open/close, IPC stubs, and X11 `set_bounds` resize wiring).
  - **(2) Substantive Invariant Verification**:
    - **`ViewHost::set_bounds` X11 Wiring & Lock Ordering**: `ViewHost.views` (`ViewState`) and `X11ViewHost.views` (`X11ViewState`) are separate maps. `set_bounds` takes `ViewHost.views` read guard, locks `ViewState`, and calls `X11ViewHost::resize_view` which takes `X11ViewHost.views` read guard and locks `X11ViewState`. Lock ordering is strictly unidirectional (`ViewHost -> X11ViewHost`), eliminating deadlock hazards while routing through `XMoveResizeWindow` to restore dynamic resizing on X11.
    - **`parked_if_closed` & Coordinate Safety**: Closed panels are assigned a 1px height (preventing X11 `BadValue` on 0-dimension windows) and parked at `y + PARK_OFFSET` (`16_000`). At normal window heights (~800–1440), the coordinate evaluates to ~16,800–17,440, safely below X11 protocol `INT16` upper bound (32,767).
    - **Chrome Hole & Page Column Composition**: `apply_layout` cuts an XShape hole spanning the entire column below the chrome `(sidebar_width, chrome_height, content_width, (height - chrome_height).max(0.0))`. When closed, the page fills the whole hole up to the shelf and the closed panel is parked off-screen. The chrome does not cover the page. When open, the page shrinks by 300px and the inspector sits immediately below it.
  - **(3) Tests & Verification**:
    - All 4 `hiwave-app` feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
    - `hiwave-app` unit tests: **84 passed**, 0 failed (including all 5 new tests in `inspector_panel`).
    - `hiwave-app` integration tests: **27 passed**, 0 failed (`--test-threads=1`).
    - `hiwave-app` stack test: **1 passed**.
    - `rustkit-viewhost`: **46 passed**, 0 failed.
- **Decisions for Atlas**: PR #104 is CLEAR to merge to `develop` at head `95030257b4f9b5cdc9f8fa11358747be72ab8cd5`.

## 2026-11-07 11:50 Castor PR #104 Landed & Standing By

- **develop**: Advanced to `2fa17359` with PR #104 merge (`Merge pull request #104 from hiwavebrowser/talos/i1a-inspector`).
- **Fleet Updates**:
  - Atlas broadcast #1188: macOS PR #979 opened adopting Linux patterns from #974 findings (registry assertion `cfg!(any(macos, linux))` and unique retry text indexing per #102).
  - Atlas broadcast #1189: Draft #981 opened for `clip-path` inset round percent resolving against reference box.
  - Talos completed local gates on `talos/s6-sync-2026-11-07` (`e123b59b`), bringing in Batch 35 (`3b4561e3..0cb89fbe`) and dropping the Linux-specific shape memo copy.
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos to publish the S6 sync PR for R1 review.

## 2026-11-07 12:30 Castor R1 on PR #105 (Stage S6: hiwave-macos Sync 3b4561e3..0cb89fbe)

- **PR**: `hiwavebrowser/hiwave-linux#105` (`talos/s6-sync-2026-11-07`, author: Talos).
- **Head SHA**: `241af075967aaf7427b1f554606078ffdb9a69ce` (develop merged in). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #46 to exchange.
- **Land**: Merged by Atlas into `develop` as `c63b5d6b`.
- **Audit Findings**:
  - **(1) Scope**: 2 files touched (`crates/rustkit-layout/src/text.rs` and `crates/rustkit-renderer/src/scroll_cost_tests.rs`). Brings in Batch 35 from macOS develop `0cb89fbe`:
    - #974 (LAT12): Adopts upstream cfg-free `pub fn shape` text memo (`MEMO.with`, `size <= 0.0` guard, `webfonts::generation()` invalidation, `MISSES` counter) and removes Linux-specific duplicate copy from #100/#102. Linux FreeType implementation cleanly retained as `fn shape_uncached` under `#[cfg(target_os = "linux")]`. Declared test divergence in `a_new_web_font_set_empties_the_memo` (`cfg!(any(target_os = "macos", target_os = "linux"))`) matches upstream PR #979.
    - #975 (T30): `scroll_cost_tests.rs` introduces `BufferLimitCase` dynamically adjusting limits on software/CPU adapters (`DeviceType::Cpu`), avoiding D3D12 WARP and llvmpipe queue timeouts while preserving 4:1 buffer-to-batch ratio.
  - **(2) Verification**:
    - `rustkit-layout`: `shape_memo_tests` 3 passed, 0 failed.
    - `rustkit-renderer`: `scroll_cost_tests` 10 passed, 0 failed, 2 ignored.
- **Decisions for Atlas**: None (landed).

## 2026-11-07 12:35 Castor R1 on PR #106 (Stage I1b: Inspector Data for WebKit Content)

- **PR**: `hiwavebrowser/hiwave-linux#106` (`talos/i1b-inspector-data`, author: Talos).
- **Head SHA**: `1aaca08f2901d773c4917ecbaf30cf8f196c9917` (develop merged in). Base: `develop`.
- **Verdict**: **R1 CLEAR** posted on GitHub and broadcast #46 to exchange.
- **Land**: Merged by Atlas into `develop` as `a62900b0`.
- **Audit Findings**:
  - **(1) Scope**: 2 files (`crates/hiwave-app/src/main.rs` +245/-6, `crates/hiwave-app/src/ui/inspector_inject.js` +434). Wires DOM tree, styles, picker, and console for WebKit content.
  - **(2) Substantive Invariant Verification**:
    - **macOS `main.rs:6155-6340` Port Parity & Escaping**: All 16 `UserEvent` arms ported accurately. Path escaping (`path.replace("\\", "\\\\").replace("'", "\\'")`) and eval code escaping (`code.replace("\\", "\\\\").replace("'", "\\'").replace("\n", "\\n").replace("\r", "\\r")`) verified safe against injection.
    - **Content Architecture**: Operates via `inspector_inject.js` and IPC forwarding. Properly omits `with_rustkit_content!` which is deferred to I1b-2.
  - **(3) Verification**:
    - All 4 `hiwave-app` feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
    - `hiwave-app` unit tests: **84 passed**, 0 failed (single-threaded).
- **Decisions for Atlas**: None (landed). Develop advanced to `a62900b0`.

## 2026-11-07 13:30 Castor R1 on PR #107 (Stage I1b-2: Native RustKit Content Devtools Path)

- **PR**: `hiwavebrowser/hiwave-linux#107` (`talos/i1b2-devtools`, author: Talos).
- **Head SHA**: `e3f9fc5ab8cc08a92f7b3b4b8ec0a4ada4c49448`. Base: `develop`.
- **Verdict**: **R1 CLEAR at e3f9fc5a** posted on GitHub and broadcast #48 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 5 files touched (+133/-6 across `devtools_panel.rs`, `inspector_panel.rs`, `main.rs`, `rustkit_content.rs`, `webview_rustkit.rs`). Implements native developer-tools support for RustKit pages (`HIWAVE_RUSTKIT_CONTENT=1`).
  - **(2) Substantive Invariant Verification**:
    - **`ChromeUi` Trait Adaptation**: `InspectorPanel` implements `ChromeUi` (`eval`, `place` no-op, `take_keyboard`), delegating evaluation to `LazyPanel`.
    - **Pump Integration & Input Filtering**: `PumpHooks` (`on_turn`, `input_filter`) added to `rustkit_content::install_pump`. `pump_x11_input_filtered` intercepts pointer events during element picking (`MouseMove`, `MouseLeave`, `MouseUp`, `KeyDown 0x1B / Escape`, `MouseDown`), suppressing page dispatch and feeding hover/selection to devtools.
    - **Dispatch Parity (`with_devtools!`)**: Macro cleanly inspects `UnifiedContentWebView::RustKit` and routes all 9 inspector events (`InspectorHighlightElement`, `InspectorClearHighlight`, `InspectorStartPicker`, `InspectorStopPicker`, `InspectorRefreshTree`, `InspectorSelect`, `InspectorExpand`, `InspectorConsoleClear`, `InspectorConsoleEval`) to the native engine path before falling through to WebKit injection.
    - **Omission Accepted**: Omission of `inspector_page_tests.rs` (pending engine test serialization guard to avoid parallel test SIGSEGV) verified and accepted.
  - **(3) Verification**:
    - All 4 `hiwave-app` feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
    - `devtools_panel::tests`: **11 passed**, 0 failed.
    - `hiwave-app` unit tests (`--bin hiwave -- --test-threads=1`): **95 passed**, 0 failed (84 prior + 11 new).
    - `rustkit-viewhost`: **46 passed**, 0 failed.
- **Land**: Merged by Atlas into `develop` as `43ac5d62`.
- **Decisions for Atlas**: None (landed).

## 2026-11-07 15:00 Castor R1 on PR #108 (Stage S7: hiwave-macos Sync 0cb89fbe..478cee42)

- **PR**: `hiwavebrowser/hiwave-linux#108` (`talos/s7-sync-2026-11-07`, author: Talos).
- **Head SHA**: `983bf8bdd476763194949599066d512314efe34a`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 983bf8bd** posted on GitHub and broadcast #49 to exchange.
- **Land**: Merged by Atlas into `develop` as `ae1ceff0`.
- **Audit Findings**:
  - **(1) Scope**: 4 files touched (+154/-9 across `rustkit-engine/src/abspos_static_margin_tests.rs`, `rustkit-engine/src/lib.rs`, `rustkit-layout/src/lib.rs`, `tools/parity_oracle/abspos_static_margin_cases.json`). Integrates Batch 36 (#976 / CH2.B24 abspos static position margin collapsing fix).
  - **(2) Substantive Invariant Verification**:
    - **Out-of-flow Margin Isolation**: In `crates/rustkit-layout/src/lib.rs`, `cb.content.height` materializes the preceding sibling's bottom margin (`cursor_y + margin_context.pending_margin()`), while the out-of-flow child resolves against a fresh `MarginCollapseContext` with `collapse_top = false`. This guarantees the abspos box's own `margin-top` does not collapse into the in-flow sibling's margin per CSS 2.1 §8.3.1 and matches Chromium behavior. Live margin context remains intact for subsequent siblings.
    - **Declared Divergence**: `abspos_static_margin_tests.rs` un-gated for Linux (`cfg(any(target_os = "macos", target_os = "linux"))`). All 14 cases in `abspos_static_margin_cases.json` verify cleanly against Chromium 148 pins on Linux.
  - **(3) Verification**:
    - `abspos_static_margin_tests`: **1 passed**, 0 failed (14 Chromium pin cases).
    - `rustkit-layout`: **886 passed**, 0 failed, 16 ignored.
    - `rustkit-engine` lib tests: **902 passed**, 0 failed, 6 ignored (`--test-threads=1`).
    - `hiwave-app` unit tests: **84 passed**, 0 failed (`--test-threads=1`).
- **Decisions for Atlas**: None (landed). Develop advanced to `ae1ceff0`. Standing by for Talos's S8 sync PR (Batch 37 / `a8e7e28b`).

## 2026-11-07 17:35 Castor R1 on PR #109 (Stage S8: hiwave-macos Sync 478cee42..a8e7e28b)

- **PR**: `hiwavebrowser/hiwave-linux#109` (`talos/s8-sync-2026-11-07`, author: Talos).
- **Head SHA**: `8eaf75f63c6e4828a10a2e99b8a9077cc69e066e`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 8eaf75f6** posted on GitHub and broadcast #51 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 10 files touched (+521/-106 across `rustkit-bindings/src/dom.rs`, `rustkit-bindings/src/lib.rs`, `rustkit-engine/src/lib.rs`, `relayout_cause_engine_tests.rs`, `style_attr_declaration_tests.rs`, `wgate.rs`, `rustkit-layout/src/text.rs`, `scripts/tests/test_latency_baseline_from_logs.py`, `tools/latency/baseline_from_logs.py`, `tools/parity_oracle/style_attr_declaration_cases.json`). Integrates macOS Batch 37 (`a8e7e28b`):
    - #979: Upstream adopted Linux findings from S6 (`a_new_web_font_set_empties_the_memo` checking `cfg!(any(target_os = "macos", target_os = "linux"))` and unique retry text indexing per attempt). In S8, upstream's `crates/rustkit-layout/src/text.rs` is taken directly, completely eliminating the temporary declared Linux divergence.
    - #978 (LAT1b): Per-visit hold summaries now cleanly tag `view=` and `host=` in `wgate.rs` and `relayout_cause_engine_tests.rs`. Log redaction preserves privacy (query and fragment stripped, no page text in record labels).
    - #977 (CH2.E29): `el.style` drops bad url declarations per CSS Syntax 3 §4.3.6 (`hasBadUrl`, `splitDecls`); rejected inline background preserves sheet image without leaking or breaking DOM style setters.
  - **(2) Substantive Invariant Verification**:
    - **Shape Memo Parity**: Confirmed `text::shape_memo_tests` passes cleanly (3 passed). Upstream's clean integration of the Linux cfg guard aligns the codebase back to 0 divergence on the text shaping path.
    - **Oracle Parity**: All 9 cases in `tools/parity_oracle/style_attr_declaration_cases.json` verify against Chromium 148 pins on Linux.
    - **Log Redaction**: `relayout_cause` tests verify summary URLs drop query and fragment, and record labels hold no page text.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `rustkit-bindings`: `style_splits_the_shared_declaration_list_cases` **1 passed**, 0 failed.
    - `rustkit-layout`: `shape_memo_tests` **3 passed**, 0 failed.
    - `rustkit-engine`: `wgate` and `style_attr_declaration` **17 passed**, 0 failed.
    - `rustkit-engine`: `relayout_cause` **7 passed**, 0 failed.
    - `scripts/tests/test_latency_baseline_from_logs.py`: **13 passed**, 0 failed.
    - `hiwave-app` unit tests (`--bin hiwave -- --test-threads=1`): **95 passed**, 0 failed, 0 ignored.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #109 is CLEAR to merge to `develop` at head `8eaf75f63c6e4828a10a2e99b8a9077cc69e066e`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `01fe7038` (Atlas broadcast 2026-11-07T22:43). Develop advanced to `01fe7038`.

## 2026-11-07 21:00 Castor R1 on PR #110 (Inspector Page Tests & engine_test_turn Serialization Guard)

- **PR**: `hiwavebrowser/hiwave-linux#110` (`talos/inspector-page-tests`, author: Talos).
- **Head SHA**: `cab22e171f02abf76a2e7726ba1e832be1c30612`. Base: `develop`.
- **Verdict**: **R1 CLEAR at cab22e17** posted on GitHub and broadcast #52 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 3 files touched (+52/-0 across `crates/hiwave-app/src/inspector_page_tests.rs`, `main.rs`, and `webview_rustkit.rs`). Purely test-scoped under `#[cfg(test)]`. No production code changed.
  - **(2) Substantive Invariant Verification**:
    - **Lock Ordering & Acquisition Guard**: `crate::engine_test_turn()` defined in `main.rs` with poison recovery. Every one of the 8 tests in `inspector_page_tests.rs` takes `let _turn = one_engine_at_a_time();` as its first statement before `inspector()` constructs an `Engine`. Every one of the 30 tests in `webview_rustkit.rs::tests` takes `let _turn = crate::engine_test_turn();` before calling `view()` or `content_view()`. The 4 `x11_delivery_tests` only test pure input struct mappings and omit the lock.
    - **Deadlock & Await Safety**: Locks are thread-stack RAII guards on synchronous unit test threads. No test holds the turn across an await on another guarded test.
    - **Clean Isolation & Defect Prevention**: Ran on private Xvfb (`DISPLAY=:99`) with isolated scratch XDG directories (`XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`).
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave`): **103 passed**, 0 failed, 0 ignored in 89.17s (all 8 `inspector_page_tests` pass; zero SIGSEGV across parallel run).
    - `rustkit-viewhost`: **46 passed**, 0 failed.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #110 is CLEAR to merge to `develop` at head `cab22e171f02abf76a2e7726ba1e832be1c30612`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `e130009a`. Develop advanced to `e130009a`.

## 2026-11-07 23:00 Castor R1 on PR #111 (Linux Typed Text Relayout on KeyDown)

- **PR**: `hiwavebrowser/hiwave-linux#111` (`talos/key-relayout`, author: Talos).
- **Head SHA**: `2da802b20bde8f6eaeeb1c04eb43123bfd788243`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 2da802b2** posted on GitHub and broadcast #53 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 1 file touched (+4/-0 in `crates/hiwave-app/src/webview_rustkit.rs`).
  - **(2) Substantive Invariant Verification**:
    - **Parity Alignment**: In `pump_x11_input`, `self.relayout()` is added to the `X11Delivery::KeyDown` arm immediately after `self.handle_text_key(vk, &text, ctrl, shift, alt)`.
    - **Mechanism**: Page engines do not trigger full layout/paint unprompted upon key dispatch. Without sending `PageCmd::Relayout`, typed characters update control internal text/caret state but do not render on screen (leaving the address bar visually stuck on its placeholder until a forced relayout). macOS performs `view.relayout()` after handling text keys; Athena independently identified the exact same requirement on Windows (`hiwave-windows#123`). This single-call addition restores visual typing responsiveness on Linux (e.g., `abcdef` shows at 0.5s instead of staying blank).
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 94.18s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #111 is CLEAR to merge to `develop` at head `2da802b20bde8f6eaeeb1c04eb43123bfd788243`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `76af95d7`. Develop advanced to `76af95d7`.

## 2026-11-07 23:45 Castor R1 on PR #112 (Stage S9: hiwave-macos Batch 38 Sync a8e7e28b..c7209173)

- **PR**: `hiwavebrowser/hiwave-linux#112` (`talos/s9-sync-2026-11-08`, author: Talos).
- **Head SHA**: `60e813aff23b9b86b6891ac9afbf41e6f30f20cf`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 60e813af** posted on GitHub and broadcast #54 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 16 files touched (+1307/-62 across `rustkit-bindings`, `rustkit-css`, `rustkit-engine`, `rustkit-layout`, `tools/parity_oracle`). Zero conflict hunks across 2 per-file merges.
  - **(2) Substantive Invariant Verification**:
    - **#980 (Test-Only Pins)**: Integrates pins for relative offsets (`relative_offset_unit_tests.rs`), display inherit (`display_inherit_apply_tests.rs`), css display (`css_display_apply_tests.rs`), flex shorthand (`flex_shorthand_tests.rs`), scroll helpers (`element_scroll_helper_tests.rs`), border medium (`border_medium_apply_tests.rs`).
    - **#968 (CH2.E18 - Attribute Selector `i` Flag)**: Case-insensitive `i` flag supported across attribute selector matching (`attribute_flag_tests.rs`).
    - **#951 (CH2.E13b - `innerText` White-Space & Display Parity)**: In `inner_text.rs`, `space_of` reads computed `white-space`, table checks read computed `display`, and boxless `display: contents` `<p>` elements generate zero required breaks, exactly matching Chromium 143 pins.
    - **#956 (CH2.E14b - Element `var()` Unset & Linux Shorthand Adaptation)**: Custom property `var()` with no value and no fallback computes as unset. Linux declared divergence cleanly calls `Self::apply_initial_value_impl(style, &format!("{prefix}-{side}{suffix}"))` for 4-sided shorthand expansions (`margin`, `padding`, `border-width`), completely resolving E0424 compile hazards.
    - **#981 & #982 (C8d - `clip-path` Inset Round & Inlines)**: `clip_path_clip` extended to non-replaced inline boxes (`BoxType::Inline`) with vertical margin calculation matching WPT `clip-path-inline-004`. Inset round percentages correctly resolve against reference box dimensions per CSS Shapes 1 §3.1.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `rustkit-layout`: **895 passed**, 0 failed, 16 ignored (+9 new tests).
    - `rustkit-bindings`: **483 passed**, 0 failed (482 lib + 1 census).
    - `rustkit-engine` (`--lib --test-threads=1`): **924 passed**, 0 failed, 6 ignored (+20 new tests).
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 89.20s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #112 is CLEAR to merge to `develop` at head `60e813aff23b9b86b6891ac9afbf41e6f30f20cf`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `477c9ef4`. Develop advanced to `477c9ef4`.

## 2026-11-08 00:05 Castor R1 on PR #113 (Linux System Clipboard via arboard)

- **PR**: `hiwavebrowser/hiwave-linux#113` (`talos/system-clipboard`, author: Talos).
- **Head SHA**: `191d90adebe40cab783c1db50a703f36cc03388a`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 191d90ad** posted on GitHub and broadcast #55 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 3 files touched (+220/-2 across `Cargo.lock`, `crates/rustkit-engine/Cargo.toml`, `crates/rustkit-engine/src/text_input.rs`).
  - **(2) Substantive Invariant Verification**:
    - **Hook Integration & Robustness**: `system_clipboard()` routes to `LinuxClipboard` under `#[cfg(target_os = "linux")]`. Uses `arboard::Clipboard` with a static process-wide mutex guard, ensuring X11 CLIPBOARD selection owner threads stay alive across clipboard read/write events.
    - **Headless & Display Failure Fallback**: If `arboard::Clipboard::new()` fails (e.g. headless CI or display-less server runs), it logs a warning once and falls back gracefully to `linux_clipboard_fallback()`. Unit tests and in-app copy/paste proceed normally without panics.
    - **Dependency footprint**: Declared Linux divergence using `arboard` 3 (`default-features = false`, `wayland-data-control = true`) for plain text clipboard operations without multimedia/image overhead.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `rustkit-engine` clipboard unit suite: **9 passed**, 0 failed (`clipboard`, `paste`, `copy_and`, `cut`).
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 91.34s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #113 is CLEAR to merge to `develop` at head `191d90adebe40cab783c1db50a703f36cc03388a`. Ready for Atlas decision/merge.
- **Land**: Merged by Atlas into `develop` as `12f51511`. Develop advanced to `12f51511`.

## 2026-11-08 00:30 Castor R1 on PR #114 (Fix Space scrolls a page down VK_SPACE 0x20)

- **PR**: `hiwavebrowser/hiwave-linux#114` (`talos/space-scroll`, author: Talos).
- **Head SHA**: `6db51ce8488b418dd1ccadcfcb3329e96bbaeb6d`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 6db51ce8** posted on GitHub and broadcast #56 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 1 file touched (+16/-1 in `crates/rustkit-engine/src/page_protocol.rs`).
  - **(2) Substantive Invariant Verification**:
    - **Cross-Platform Normalization**: `scroll_dy_for_unhandled_key` arm expanded to include `(0x20, " ")` alongside macOS `(0, " ")` and `(0x22, _)`.
    - On macOS, Space key delivery arrives as `(0, " ")`. On Linux/X11 and Windows/Win32, Space arrives as `(0x20, " ")`. Previously, Linux and Windows fell through without issuing a `PageCmd::ScrollDy(viewport_height - 40.0)` for unhandled pages.
    - Resolves the shared space-scrolling gap directly at the engine layer (upstream row CH2.E39) without platform-specific divergence.
    - Dedicated unit test `unhandled_key_scroll_tests::space_scrolls_a_page_down_whichever_way_the_platform_delivers_it` pins macOS convention `(0, " ")`, Win32/X11 convention `(0x20, " ")`, PageDown `(0x22, _)`, and non-scrolling keys.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `rustkit-engine` unhandled key scroll tests: **1 passed**, 0 failed (`space_scrolls_a_page_down_whichever_way_the_platform_delivers_it`).
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 87.79s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #114 is CLEAR to merge to `develop` at head `6db51ce8488b418dd1ccadcfcb3329e96bbaeb6d`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `be650d51`. Develop advanced to `be650d51`.

## 2026-11-08 01:50 Castor PR #114 Landed & Standing By

- **develop**: Advanced to `be650d51` with PR #114 merge (`Merge pull request #114 from hiwavebrowser/talos/space-scroll`).
- **Fleet Updates**:
  - Atlas broadcast #1216: PR #114 merged on Castor's R1; upstream row CH2.E39 stays open to track eliminating divergence in engine. target=_blank finding confirmed as shared product gap (row CH2.E40).
  - Atlas broadcast `atlas-batch39`: `hiwave-macos develop` advanced to `8ebc1b79` (Batch 39: #958 test pins, #983 CH2.B25 inline abspos static position, #986 CH2.B18c author border-width on input).
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's next incoming Linux package (Stage S10 Batch 39 sync `c7209173..8ebc1b79`).

## 2026-11-08 03:10 Castor R1 on PR #115 (Stage S10: hiwave-macos Batch 39 Sync c7209173..8ebc1b79)

- **PR**: `hiwavebrowser/hiwave-linux#115` (`talos/s10-sync-2026-11-08`, author: Talos).
- **Head SHA**: `93ccf4d24ff62ceeea93dc227a43d5fe1bcefe4a`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 93ccf4d2** posted on GitHub and broadcast #58 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 12 files touched (+671/-15 across `rustkit-bindings`, `rustkit-css`, `rustkit-engine`, `rustkit-layout`, `tools/parity_oracle`). Synchronizes `hiwave-macos develop` Batch 39 (`c7209173..8ebc1b79`):
    - **#958**: Test-only pins for LAT2 dirty gate, LAT4b SVG parts, and form control content box.
    - **#983 (CH2.B25)**: Inline-level abspos static position stays on the line across block formatting contexts and grid containers.
    - **#986 (CH2.B18c)**: Author `border-width` on `<input>` sets the painted control frame dimensions.
  - **(2) Substantive Invariant Verification & Linux Adaptation**:
    - **Exhaustive Field Guard Fix (E0027)**: #983 added `abspos_was_inline_level: bool` to `ComputedStyle`. The two Linux-carried `rustkit-css` regression tests (`mod computed_style_field_guards`) destructure `ComputedStyle` with exhaustive field matching to guard inheritance fall-through. Fix commit `2525c6ad` correctly categorizes `abspos_was_inline_level` in both tests (default `false`), completely resolving E0027 compile failure while preserving inheritance invariants.
    - **Per-File Merges**: 3 clean per-file merges (`rustkit-css`, `rustkit-engine`, `rustkit-layout`) with zero conflict hunks.
    - **Develop Merge**: Integration branch `develop` (including PR #113 clipboard and PR #114 Space scroll fix) cleanly merged in at head `93ccf4d2`.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `rustkit-css`: **154 passed**, 0 failed (E0027 resolved).
    - `rustkit-layout`: **901 passed**, 0 failed, 16 ignored (+6 new tests over S9).
    - `rustkit-bindings`: **489 passed** (488 lib + 1 census), 0 failed.
    - `rustkit-engine` (`--lib --test-threads=1`): **928 passed**, 0 failed, 6 ignored (+4 new tests over S9: includes #114 space scroll and #983 abspos static rest tests).
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 89.17s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #115 is CLEAR to merge to `develop` at head `93ccf4d24ff62ceeea93dc227a43d5fe1bcefe4a`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `5c3f1496`. Develop advanced to `5c3f1496`.

## 2026-11-08 07:50 Castor PR #115 Landed & Standing By

- **develop**: Advanced to `5c3f1496` with PR #115 merge (`Merge pull request #115 from hiwavebrowser/talos/s10-sync-2026-11-08`).
- **Fleet Updates**:
  - Atlas broadcast #1222: PR #115 merged on Castor's R1; Linux `develop` at `5c3f1496`.
  - Upstream `hiwave-macos`: PR #993 (CH2.B30 UA em margins) cleared; #992 / #994 progressing towards Batch 40.
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's next incoming Linux package.

## 2026-11-08 08:25 Castor R1 on PR #116 (Linux Chrome Board S4 & layout_oracle_gate)

- **PR**: `hiwavebrowser/hiwave-linux#116` (`talos/chrome-board-linux`, author: Talos).
- **Head SHA**: `3d6e3958910fadb4f13b58d74a516b5e08d6310f`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 3d6e3958** posted on GitHub and broadcast #60 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 4 files touched (+3893/-0 across `scripts/layout_oracle_gate.py`, `scripts/tests/test_layout_oracle_gate.py`, `trench/chrome/BOARD-linux.md`, and `trench/chrome/board-linux.json`). Strictly tooling and measurement artefacts; zero engine or product code modified.
  - **(2) Substantive Audit**:
    - Adds missing `scripts/layout_oracle_gate.py` and unit test from macOS `develop` to resolve `ModuleNotFoundError` during `scripts/chrome_board.py` geometry comparison.
    - S4 baseline run scored against pinned Chrome for Testing `148.0.7778.216` under private Xvfb `:99` with isolated scratch XDG directories.
    - All 42 rows successfully scored (0 capture errors).
    - Under PLAN-z CH0 gate rules (`pixel_diff_pct <= 0.5%` OR `geometry_diffs == 0 and pixel_diff_pct <= 1.5%`): **16/42 rows green** (chrome 7/21, shelf 6/6, inspector 3/6, settings 0/3, report 0/6).
    - Headline coincidence with macOS 16/42 is explicitly acknowledged as a coincidence (independent baseline for Linux fonts/FreeType rasterizer per Atlas ruling 1).
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `python3 -m pytest scripts/tests/test_chrome_board.py scripts/tests/test_layout_oracle_gate.py`: **46 passed in 0.20s** (19 board + 27 oracle gate).
    - `trench/chrome/board-linux.json` independently parsed: 42 rows present, exactly 16 evaluate to green.
    - Working tree clean at head `3d6e3958`.
- **Decisions for Atlas**: PR #116 is CLEAR to merge to `develop` at head `3d6e3958910fadb4f13b58d74a516b5e08d6310f`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `e2865e43`. Develop advanced to `e2865e43`.

## 2026-11-08 08:50 Castor PR #116 Landed & Standing By

- **develop**: Advanced to `e2865e43` with PR #116 merge (`Merge pull request #116 from hiwavebrowser/talos/chrome-board-linux`).
- **Fleet Updates**:
  - Atlas broadcast #1228: PR #116 merged on Castor's R1; Batch 40 gating on `hiwave-macos`.
  - Next Linux step: Talos font pinning task per Atlas ruling (2) to align CfT and HiWave fontconfig/hinting before row fixes.
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's next incoming Linux package.

## 2026-11-08 09:25 Castor R1 on PR #118 (chrome board: font pin --fonts for both arms)

- **PR**: `hiwavebrowser/hiwave-linux#118` (`talos/board-font-pin`, author: Talos).
- **Head SHA**: `400d174b9a2fa438270d111b5e401790959bd0a2`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 400d174b** posted on GitHub and broadcast #62 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 2 files touched (+39/-0 across `scripts/board-fonts.conf` and `scripts/chrome_board.py`). Tooling-only S4 follow-up implementing Atlas ruling (2). Zero engine or product code modified.
  - **(2) Substantive Audit**:
    - `scripts/board-fonts.conf` explicitly binds generic font families across both Chrome for Testing and RustKit `parity-capture` (`sans-serif` and `system-ui` -> `Noto Sans`, `serif` -> `Noto Serif`, `monospace` -> `DejaVu Sans Mono`), locking hinting to `hintslight`, antialiasing to true, and disabling LCD subpixel filtering to ensure determinism across machines.
    - `scripts/chrome_board.py` wires `--fonts FONTS_CONF` into `os.environ` and Node subprocess environment (`NODE_ENV`), and embeds resolved font families in `board.json` metadata via `resolved_fonts()`.
    - Pinned run verified at identical 16/42 green, confirming current host defaults match the pin and establishing the stable baseline for future row diff investigations.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `python3 -m pytest scripts/tests/test_chrome_board.py scripts/tests/test_layout_oracle_gate.py`: **46 passed in 0.21s**.
    - Font resolution tested directly with `scripts/board-fonts.conf`: matches `Noto Sans`, `Noto Serif`, `DejaVu Sans Mono` as expected.
    - Working tree clean at head `400d174b`.
- **Decisions for Atlas**: PR #118 is CLEAR to merge to `develop` at head `400d174b9a2fa438270d111b5e401790959bd0a2`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `2d0a2172`. Develop advanced to `2d0a2172`.

## 2026-11-08 09:50 Castor PR #118 Landed & Standing By

- **develop**: Advanced to `2d0a2172` with PR #118 merge (`Merge pull request #118 from hiwavebrowser/talos/board-font-pin`).
- **Fleet Updates**:
  - Atlas broadcast #1230: PR #118 merged on Castor's R1; Windows #131 merged on Pollux's R1. Batch 40 re-forming around #993 alone (Prometheus hold on #994 CRLF continuation regression).
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's next incoming Linux package.

## 2026-11-08 13:10 Castor R1 on PR #119 (Stage S11: hiwave-macos Batch 40 Sync 8ebc1b79..27129415)

- **PR**: `hiwavebrowser/hiwave-linux#119` (`talos/s11-sync-2026-11-08`, author: Talos).
- **Head SHA**: `4dc082498466089d358af23a11a39bf91ab6d3c7`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 4dc08249** posted on GitHub and broadcast #64 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 2 files touched (+138/-29 across `crates/rustkit-engine/src/lib.rs` and `crates/rustkit-engine/src/ua_em_margin_tests.rs`). Synchronizes `hiwave-macos develop` Batch 40 (`8ebc1b79..27129415`):
    - **#993 (CH2.B30)**: UA block margins in `em` resolved against element computed font size.
  - **(2) Substantive Audit**:
    - Clean 3-way merge with zero conflict hunks, matching the macOS delta exactly.
    - Replaces fixed px block margins (calculated assuming 16px font size) for headings (`h1`-`h6`), paragraphs (`p`), `pre`, lists (`ul`, `ol`, `dl`), quotes (`blockquote`), `hr`, and `figure` with `Length::Em` margins per HTML/CSS specifications.
    - Eliminates font-size scaling drift on elements with non-16px font sizes (e.g. `.subtitle { font-size: 0.9rem }` on `<p>` resolves to 14.4px margins instead of remaining pinned to 16.0px).
    - Oracles in `ua_em_margin_tests.rs` pin exact `getComputedStyle` margin matching against Chrome for Testing 148 across 16 element variants.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `rustkit-engine` `ua_em_margin_tests`: **2 passed**, 0 failed.
    - `rustkit-engine` lib suite (`--test-threads=1`): **928 passed**, 0 failed, 6 ignored.
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 89.51s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #119 is CLEAR to merge to `develop` at head `4dc082498466089d358af23a11a39bf91ab6d3c7`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `ab579d43`. Develop advanced to `ab579d43`.

## 2026-11-08 13:55 Castor PR #119 Landed & Standing By

- **develop**: Advanced to `ab579d43` with PR #119 merge (`Merge pull request #119 from hiwavebrowser/talos/s11-sync-2026-11-08`).
- **Fleet Updates**:
  - Atlas merged PR #119 into `hiwave-linux develop` (`ab579d43`).
  - Talos reported font-pinned chrome board on S11 tree (`ab579d43`) scored 20/42 green (+4 rows, settings/report em margin fixes) (Talos broadcast #430).
  - macOS Batch 41 landed on `hiwave-macos develop`: `27129415..022aa2fa` (#992 LAT13 layer submits).
  - Upstream macOS develop fetched to `022aa2fa`.
  - Pete paused new macOS work; Windows and Linux continue.
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's Stage S12 sync PR (`27129415..022aa2fa`) or chrome board follow-up packages.

## 2026-11-08 16:15 Castor R1 on PR #120 (Stage S12: hiwave-macos Batch 41 Sync 27129415..022aa2fa)

- **PR**: `hiwavebrowser/hiwave-linux#120` (`talos/s12-sync-2026-11-08`, author: Talos).
- **Head SHA**: `c269d4c086e04a38fbdd41b59e9deadba7d575a1`. Base: `develop`.
- **Verdict**: **R1 CLEAR at c269d4c0** posted on GitHub and broadcast #66 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 8 files touched (+1618/-422 across `crates/rustkit-engine/src/layer_frame_time_tests.rs`, `crates/rustkit-engine/src/lib.rs`, `crates/rustkit-engine/src/presenter.rs`, `crates/rustkit-renderer/src/frame_stats.rs`, `crates/rustkit-renderer/src/glyph.rs`, `crates/rustkit-renderer/src/layer_cost_tests.rs`, `crates/rustkit-renderer/src/lib.rs`, `crates/rustkit-renderer/src/scroll_cost_tests.rs`). Synchronizes `hiwave-macos develop` Batch 41 (`27129415..022aa2fa`):
    - **#992 (LAT13)**: Single-encoder frame recording, vertex/index arena buffer, shared layer-texture slots, mid-frame glyph atlas reset discards/re-records, idle layer slot trimming.
  - **(2) Substantive Audit**:
    - **`glyph.rs` Line Ending & Mid-Frame Reset**: Cleanly resolved the CRLF vs LF conflict in `glyph.rs`. Applied upstream's 15-line `start_over`/`atlas_fill` logic while preserving LF and Linux FreeType backend caching (`ft_backend`), Skia subpixel positioning (`pen_and_phase`), and Linux emoji detection intact.
    - **Linux Test Serialization Guard (`GPU_SERIAL`)**: Verified test-only `#[cfg(test)] pub(crate) static GPU_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());` in `crates/rustkit-renderer/src/lib.rs`, held during GPU test execution in `layer_cost_tests::gpu()` and `scroll_cost_tests::gpu()`. This completely resolves the Vulkan loader driver crash (`SIGSEGV` in `dlclose` under `vkEnumeratePhysicalDevices` on NVIDIA drivers) when running parallel tests, leaving production code bit-identical.
    - **Timing Tests**: Confirmed `layer_frame_time_tests.rs` carries `#[ignore = "a timing, run by hand"]` matching macOS/Windows conventions.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `gate 0` (`cargo check -p rustkit-engine --features headless`): **green** (0 errors).
    - `rustkit-renderer` lib parallel tests: **152 passed**, 0 failed, 3 ignored in 76.20s.
    - `rustkit-engine` lib suite (`--test-threads=1`): **928 passed**, 0 failed, 6 ignored in 409.31s.
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 87.70s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #120 is CLEAR to merge to `develop` at head `c269d4c086e04a38fbdd41b59e9deadba7d575a1`. Ready for Atlas merge.
- **Land**: Merged by Atlas into `develop` as `36951409`. Develop advanced to `36951409`.

## 2026-11-08 16:55 Castor PR #120 Landed & Standing By

- **develop**: Advanced to `36951409` with PR #120 merge (`Merge pull request #120 from hiwavebrowser/talos/s12-sync-2026-11-08`).
- **Fleet Updates**:
  - Atlas merged PR #120 into `hiwave-linux develop` (`36951409`).
  - Talos reported the upstream defect regarding `GPU_SERIAL` and parallel Vulkan loader races on NVIDIA Linux (Talos broadcast #431).
  - macOS Batch 42 landed on `hiwave-macos develop`: `022aa2fa..cab86bb9` (PR #996 Cursor coverage portable pins & `scrolling_chain_ids` helper extraction) (Atlas broadcast #1253).
  - Upstream macOS develop fetched locally to `cab86bb9`.
- **Status**: Zero open PRs on `hiwave-linux`. Standing by for Talos's Stage S13 sync PR (`022aa2fa..cab86bb9`) or chrome board follow-up packages.

## 2026-11-08 18:55 Castor R1 on PR #121 (Stage S13: hiwave-macos Batch 42 Sync 022aa2fa..cab86bb9)

- **PR**: `hiwavebrowser/hiwave-linux#121` (`talos/s13-sync-2026-11-08`, author: Talos).
- **Head SHA**: `7f0fbd275ed3c3728e9fa91014b151c4bc7efb30`. Base: `develop`.
- **Verdict**: **R1 CLEAR at 7f0fbd27** posted on GitHub and broadcast #68 to exchange.
- **Audit Findings**:
  - **(1) Scope**: 5 files touched (+352/-9 across `crates/rustkit-bindings/src/dom.rs`, `crates/rustkit-bindings/src/geometry_tests.rs`, `crates/rustkit-bindings/src/inner_text.rs`, `crates/rustkit-bindings/src/web_history_tests.rs`, `crates/rustkit-engine/src/style_attr_declaration_tests.rs`). Synchronizes `hiwave-macos develop` Batch 42 (`022aa2fa..cab86bb9`):
    - **#996 (Cursor coverage pins)**: `scrolling_chain_ids` helper extraction; portable pins for hash no-op, geometry, white-space/table display, `var()` unset.
  - **(2) Substantive Audit**:
    - Clean 3-way merge matching macOS Batch 42 delta byte-for-byte with 0 conflict hunks.
    - Verified `scrolling_chain_ids` helper extraction is behaviour-preserving.
    - All 7 new portable pins across `geometry_tests.rs`, `inner_text.rs`, `web_history_tests.rs`, and `style_attr_declaration_tests.rs` verify against Chromium 148 pins on Linux.
  - **(3) Verification Evidence on Linux (`legion5`, x86_64)**:
    - `gate 0` (`cargo check -p rustkit-engine --features headless`): **green** (0 errors).
    - `rustkit-bindings` test suite: **495 passed**, 0 failed (494 lib + 1 census).
    - `rustkit-engine` `style_attr_declaration_tests`: **4 passed**, 0 failed.
    - `hiwave-app` unit tests (parallel `cargo test --bin hiwave` under Xvfb `:99` and scratch XDG profile): **103 passed**, 0 failed, 0 ignored in 87.54s.
    - All 4 feature configurations compile cleanly (`default`, `native-linux`, `webview-fallback`, `rustkit`).
- **Decisions for Atlas**: PR #121 is CLEAR to merge to `develop` at head `7f0fbd275ed3c3728e9fa91014b151c4bc7efb30`. Ready for Atlas merge.

























