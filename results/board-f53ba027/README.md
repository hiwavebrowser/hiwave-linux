# Linux chrome board at f53ba027, the V1.6.0 release candidate

This folder is the receipt for the full Linux chrome board, run on hiwave-linux #157 at head **f53ba027**. That head is the S26 sync (hiwave-macos develop 2f70f02f..b60da7f5) merged with develop a34e2491, which already contains #150 and #156.

## How it was run

- **Command:** `scripts/chrome_board.py --fonts scripts/board-fonts.conf --parity-capture <release parity-capture built from f53ba027>`
- **Chrome:** the pinned Chrome for Testing 148.0.7778.216.
- **Rows:** 42 (5 pages × their states × 3 viewport sizes).
- **Machine:** legion5. No display; the board's own Chrome and the RustKit capture are both headless.

## Results

| | green | mean geometry diffs | mean pixel diff |
|---|---|---|---|
| batch 111 develop (28c50d08) | 22/42 | 76.12 | 0.854% |
| develop + #150 | 23/42 | 60.26 | 0.599% |
| develop + #150 + #156 | 42/42 | 3.00 | 0.286% |
| **f53ba027 (+ S26)** | **38/42** | **2.21** | **0.334%** |

**By page at f53ba027:**

| page | geometry diffs | notes |
|---|---|---|
| chrome | 0 | |
| report | 0 | |
| settings | 0 | |
| shelf | 0 | |
| inspector | 93 | not shipped |

**Correction (2026-11-21):** this receipt first said 42/42 and "every row is green". The board's own summary line for this run says **38/42**, and that is the number. Four report rows are red: weekly at 1024x700, 1280x800 and 1600x1000 (0.85%, 0.83%, 0.65% pixel), and default at 1024x700 (0.53%).
- They have 0 geometry diffs, but each has one **join failure**: `#error-container` is a box in Chrome and has none in RustKit (`missing_box`). The board's geometry rule needs 0 geometry diffs AND 0 join failures (`chrome_board.green_rule`), so above 0.5% pixel they are red.
- My row analysis applied the pixel and geometry parts of the rule and left out join failures; I reported its count, not the board's.
- On the same rows, Chrome logs a page error (`Cannot read properties of null (reading 'appendChild')`); whether that is why the two arms differ on `#error-container` is not yet checked.

## Which change carried what

- **#156 carried most of the gain** (60.26 → 3.00, and 23 → 42 green). It made Linux's font metrics round the way Blink does: each face's ascent and descent are rounded, then a descent that was rounded down borrows one pixel from the ascent. It also measures the bitmap emoji face from its `hhea`.
  - Before #156, the baseline sat 1 px low for many face and size pairs, and that 1 px chained down every page.
- **#150** chose fallback faces by `Emoji_Presentation` and the variation selector, and united the fallback faces into the line box. That moved 76.12 → 60.26.
- **The S26 sync** (upstream batches 112–115) took the last 9 chrome-page diffs: the collapsed `<select>` width (hiwave-macos #1369). It also took the report page's 24: Chart.js charts that had been 0×0 now lay out at Chrome's 1150×300 and draw.
  - The report page's pixel diff rose from 0.28% to 0.61%. That's the drawn chart strokes, which are now compared where empty space was compared before. That rise took four report rows over 0.5% pixel, and with the join failure above they are red (corrected; this line first said every report row was still green).

## Files

- `BOARD.md` and `board.json`: the board's own summary and per-row results.
- `rows/<page>/<state>/<size>/`:
  - `layout-rects.json.gz`: Chrome's element rects.
  - `rustkit-layout.json.gz`: RustKit's layout tree.
  - `chrome.json`: the Chrome capture's metadata.
  - `diff/attribution.json`: the pixel diff's attribution.
  - The rects are gzipped. `scripts/layout_oracle_gate.compare_case(row, chrome, rustkit)` recomputes each row's geometry result from the two unzipped files.
- **Not included:** screenshots, and `computed-styles.json` (it isn't geometry, and it's 9 MB).
