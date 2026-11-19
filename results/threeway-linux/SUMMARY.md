# Three-way rendering comparison: Linux column

RustKit on Linux vs Chrome on Linux. Measured on legion5, hiwave-linux develop **a5f9ca21** (= hiwave-macos develop 329e5d10, batch 109, plus Linux's own changes).

## Environment

- **Chrome:** Chrome for Testing 148.0.7778.216 linux64, pinned (`gate-runner/tools/cft`).
- **RustKit:** release `parity-capture` built from a5f9ca21 (sha256 prefix 73f96f1a9ffd6ce2), headless, DISPLAY unset.
- **Campaign:** `gate-runner/campaign.py`. Baselines are Linux Chrome 148 renders stored in hiwave-linux at pin fbe34035, `renders/linux/079d3b6/builtins`, at 1280x800, DPR 1.
- **Board:** `scripts/chrome_board.py --fonts scripts/board-fonts.conf`. The pinned fontconfig is used by both browsers, and the board captures Chrome live with the pinned CfT.
- **Network:** local fixtures only; no live sites.

## Metrics (per pair, then the mean over the set)

- `pixels_over_8_pct`: the share of pixels where ANY RGB channel differs by more than 8 (`realsite_board.py`'s definition).
- `mean_channel_diff`: the mean absolute RGB channel difference over all pixels, on a 0-255 scale.
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

## Linux-specific notes

- **Fonts.** RustKit on Linux shapes and rasterises with FreeType; Chrome on Linux uses Skia over FreeType/fontconfig. The board pins both to `scripts/board-fonts.conf`. The campaign's Chrome baselines are the stored renders at pin fbe34035; their capture-time font setup is not re-verified here. On the text-heavy cases (`about`, `article-typography`, `new_tab`) much of `pixels_over_8` is plausibly text anti-aliasing, but that is not measured separately.
- **DPR.** Linux baselines are DPR 1, while the Mac captures at DPR 2. Compare RustKit-Linux with RustKit-Mac at matching sizes.
- **Known Linux test failures** (`scripts/linux_known_failures.json`) are engine-test pins, not rendering-set inputs. None of them is in these sets.

## Not done here

- **The websuite set:** the '36 websuite fixtures' set is not identified yet (asked on the exchange).
- **The Mac and Windows columns.**

## Files

- `campaign.json`, `chrome_board.json`: the summary and per-case rows.
- `rustkit/campaign/<case>.png`, `rustkit/board/<page>__<state>__<size>.png`: the raw RustKit frames, for diffing RustKit-Linux against RustKit-Mac.
- `threeway_metrics.py`: the metric code.
