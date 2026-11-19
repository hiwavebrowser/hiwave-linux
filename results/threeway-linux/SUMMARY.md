# Three-way rendering comparison: Linux column

Three sets: campaign 26, websuite cases + micro 29, chrome board 42.

RustKit on Linux vs Chrome on Linux. Measured on legion5, hiwave-linux develop **a5f9ca21** (= hiwave-macos develop 329e5d10, batch 109, plus Linux's own changes).

**Re-measured at batch 111 (2026-11-19):** hiwave-linux develop **b37be7fd** carries hiwave-macos develop 2f70f02f, the engine version shared by all three platforms. All three sets were rerun there (release `parity-capture` sha256 prefix de1b835e942cd02b; same Chrome, fonts and drivers):
- every metric is identical to the batch-109 run on all 97 pairs;
- all 97 RustKit frames are pixel-identical to the ones stored here.

So the numbers and frames below stand for batch 111 too.

## Environment

- **Chrome:** Chrome for Testing 148.0.7778.216 linux64, pinned (`gate-runner/tools/cft`).
- **RustKit:** release `parity-capture` built from a5f9ca21 (sha256 prefix 73f96f1a9ffd6ce2), headless, DISPLAY unset.
- **Campaign:** `gate-runner/campaign.py`. Baselines are Linux Chrome 148 renders stored in hiwave-linux at pin fbe34035, `renders/linux/079d3b6/builtins`, at 1280x800, DPR 1.
- **Board:** `scripts/chrome_board.py --fonts scripts/board-fonts.conf`. The pinned fontconfig is used by both browsers, and the board captures Chrome live with the pinned CfT.
- **Network:** local fixtures only; no live sites.

## Metrics (per pair, then the mean over the set)

- `pixels_over_8_pct`: the share of pixels where ANY RGB channel differs by more than 8. This definition is mine, accepted by Atlas as PROVISIONAL. An earlier note cited `realsite_board.py` for it, which was wrong: that line measures the share of pixels differing from a page's dominant colour (a blank-page check), not a pair difference. The Mac column is to be computed with the same script.
- `mean_channel_diff`: the mean absolute RGB channel difference over all pixels, on a 0-255 scale (provisional, as above).
- **Board:** `geometry_diffs` and `board_green` come from the board's own `board.json`.
- **Campaign:** the tool's own pixelmatch `%` is kept as `campaign_pixelmatch_pct`.
- **Falsifier:** a pair compared with itself gives 0 / 0; a +10 shift on every channel gives 99.9% / 9.99.

Both are computed by `threeway_metrics.py` (shown below) from the saved image pairs. No image pair differed in size.

## Campaign (26 cases)

- **Mean `pixels_over_8_pct`:** 2.0048%
- **Mean `mean_channel_diff`:** 1.2423
- **Campaign pixelmatch mean 1.0954%.**
- **Pass counts at `pixels_over_8_pct` <=** 0.5% / 1% / 2%: 2 / 6 / 19 of 26. These thresholds are provisional until the Mac's pass threshold is confirmed.

Worst 10 by `pixels_over_8_pct`:

| case | pixels > 8 | mean channel diff |
|---|---|---|
| `about` | 8.1848% | 2.722 |
| `new_tab` | 6.245% | 1.3856 |
| `article-typography` | 5.7142% | 5.8694 |
| `form-controls` | 3.2208% | 3.869 |
| `chrome_rustkit` | 2.475% | 0.7743 |
| `card-grid` | 2.4707% | 1.389 |
| `settings` | 2.026% | 1.4665 |
| `backgrounds` | 1.9622% | 1.3414 |
| `css-selectors` | 1.8989% | 1.175 |
| `specificity` | 1.8587% | 1.2239 |

## Chrome board (42 rows)

- **Mean `pixels_over_8_pct`:** 1.6401%
- **Mean `mean_channel_diff`:** 0.9173
- **Board's own rule: 22/42 green; mean geometry diffs 76.12.**
- **Pass counts at `pixels_over_8_pct` <=** 0.5% / 1% / 2%: 11 / 22 / 30 of 42. These thresholds are provisional until the Mac's pass threshold is confirmed.

Worst 10 by `pixels_over_8_pct`:

| case | pixels > 8 | mean channel diff |
|---|---|---|
| `chrome/address_focused/1024x700` | 5.0932% | 3.023 |
| `chrome/hover_tab2/1024x700` | 4.9068% | 2.7738 |
| `chrome/default/1024x700` | 4.9009% | 2.7936 |
| `chrome/find_open/1024x700` | 4.7461% | 2.3347 |
| `chrome/address_focused/1280x800` | 4.1929% | 2.4807 |
| `chrome/hover_tab2/1280x800` | 3.9624% | 2.202 |
| `chrome/default/1280x800` | 3.9583% | 2.2158 |
| `chrome/find_open/1280x800` | 3.8562% | 1.9405 |
| `chrome/address_focused/1600x1000` | 3.0129% | 1.7556 |
| `chrome/hover_tab2/1600x1000` | 2.7854% | 1.4937 |

## Websuite cases + micro (29 pages)

- **Mean `pixels_over_8_pct`:** 1.3741%
- **Mean `mean_channel_diff`:** 1.0322
- **Pass counts at `pixels_over_8_pct` <=** 0.5% / 1% / 2%: 5 / 9 / 26 of 29 (provisional thresholds).
- **Pages:** `websuite/cases/*/index.html` (8) and `websuite/micro/**/*.html` (21).
  - 21 are captured at the campaign registry's size; the other 8, which are not in the registry, at 1280x800.
  - Both arms run under `scripts/board-fonts.conf`.
- **RustKit:** `parity-capture --html-file`, which injects `baselines/common/parity-reset.css` into micro pages.
- **Chrome:** the deterministic context of `capture_baseline.mjs` (pinned CfT 148), with the same parity reset injected into micro pages once `<html>` exists (`capture_chrome_websuite.mjs`). The Chrome frames are kept under `chrome/websuite/`, because 8 of these pages have no stored baseline.

Worst 10 by `pixels_over_8_pct`:

| case | pixels > 8 | mean channel diff |
|---|---|---|
| `cases__article-typography` | 5.7069% | 5.8744 |
| `micro__form-controls` | 3.2252% | 3.8723 |
| `cases__card-grid` | 2.4724% | 1.4349 |
| `micro__image-intrinsic-size` | 1.9748% | 1.1912 |
| `micro__backgrounds` | 1.9618% | 1.3383 |
| `cases__css-selectors` | 1.8906% | 1.1891 |
| `micro__specificity` | 1.8608% | 1.2238 |
| `micro__selectors-combinators` | 1.7631% | 0.7236 |
| `micro__pseudo-classes` | 1.6756% | 1.5185 |
| `micro__selectors-specificity` | 1.5063% | 0.696 |

## Finding: the parity reset never applies in the oracle's own capture

`tools/parity_oracle/deterministic.mjs` (upstream's, synced) injects `parity-reset.css` for micro pages through an init script that calls `document.documentElement.appendChild`. Init scripts run before `<html>` exists, so it throws `Cannot read properties of null (reading 'appendChild')`, and the reset is silently not applied. Every Chrome capture through it logged that error.

**Evidence.** Live captures of the 21 pages that also have a stored Linux baseline (pin fbe34035) were compared with those baselines:

| Capture | Result vs the stored baselines |
|---|---|
| Through the tool | `combinators` 11.377%, `gradient-no-radius` 8.577%, `gradient-radius-only` 7.245% of pixels over 8 |
| With the reset injected once `<html>` exists | 0.000%, 0.000%, 0.077% |
| The other 18 pages | at most 0.33% either way |

RustKit (which injects the reset) is about 1.2-1.5% off the stored baselines on those three, and 7.5-11.4% off the tool's capture. So the stored baselines were made with the reset applied, and the tool as it is today would regenerate them without it. **Not fixed here; reported upstream for a decision.**

## Linux-specific notes

- **Fonts.** RustKit on Linux shapes and rasterises with FreeType; Chrome on Linux uses Skia over FreeType/fontconfig. The board pins both to `scripts/board-fonts.conf`. The campaign's Chrome baselines are the stored renders at pin fbe34035; their capture-time font setup is not re-verified here. On the text-heavy cases (`about`, `article-typography`, `new_tab`) much of `pixels_over_8` is plausibly text anti-aliasing, but that is not measured separately.
- **DPR.** Linux baselines are DPR 1, while the Mac captures at DPR 2. Compare RustKit-Linux with RustKit-Mac at matching sizes.
- **Known Linux test failures** (`scripts/linux_known_failures.json`) are engine-test pins, not rendering-set inputs. None of them is in these sets.

## Not done here

- **The Mac and Windows columns.**

## Files

- `campaign.json`, `chrome_board.json`, `websuite.json`: the summary and per-case rows.
- `rustkit/{campaign,board,websuite}/*.png`: the raw RustKit frames, for diffing RustKit-Linux against RustKit-Mac.
- `chrome/websuite/*.png`: the Chrome frames for set (b).
- `run_websuite.py`, `capture_chrome_websuite.mjs`: set (b)'s capture drivers.
- `threeway_metrics.py`: the metric code.
