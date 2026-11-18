/**
 * deterministic.mjs - Shared deterministic settings for Playwright Chromium captures
 *
 * This module centralizes:
 * - Launch flags (color profile, font AA behavior, background throttling)
 * - Context settings (viewport, DPR)
 * - Init scripts (parity-freeze.js, optional parity-reset.css for micro-tests)
 * - Font loading for consistent text rendering
 */

import { readFileSync, existsSync } from 'fs';
import { dirname, join, resolve } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '../..');

const FREEZE_JS_PATH = join(REPO_ROOT, 'baselines', 'common', 'parity-freeze.js');
const RESET_CSS_PATH = join(REPO_ROOT, 'baselines', 'common', 'parity-reset.css');
const FONTS_DIR = join(REPO_ROOT, 'baselines', 'common', 'fonts');

// Read the reset CSS and fix font paths to use absolute file:// URLs
function getResetCssWithAbsoluteFontPaths() {
  if (!existsSync(RESET_CSS_PATH)) return '';
  let css = readFileSync(RESET_CSS_PATH, 'utf8');
  // Replace relative font URLs with absolute file:// URLs
  // On Windows, use forward slashes in file:// URLs
  const fontsPath = FONTS_DIR.replace(/\\/g, '/');
  css = css.replace(/url\(['"]?\/baselines\/common\/fonts\//g, `url('file:///${fontsPath}/`);
  css = css.replace(/format\('truetype'\)\s*;/g, `format('truetype');`);
  return css;
}

const RESET_CSS = getResetCssWithAbsoluteFontPaths();

// Campaign pin: PARITY_CHROME_PATH points at the exact Chrome for Testing
// binary recorded in trench/BASELINE-<os>.md. Playwright's bundled Chromium
// drifts with the npm package, so an output taken with it is not a pinned
// expectation. An oracle that writes expectations passes { required: true }:
// without the pin it fails. Otherwise a run without the pin says so on stderr
// and uses the bundled browser (boards and gates that do not set the pin yet;
// failing them closed is a follow-up once each sets it). Record
// browser.version() in any output.
export function getPinnedExecutablePath({ required = false, env = process.env } = {}) {
  const p = env.PARITY_CHROME_PATH;
  if (!p) {
    if (required) {
      throw new Error(
        'PARITY_CHROME_PATH is not set: point it at the pinned Chrome for Testing ' +
        '(trench/BASELINE-<os>.md); this output is a pinned expectation.'
      );
    }
    console.error(
      "deterministic.mjs: PARITY_CHROME_PATH is not set; using Playwright's bundled " +
      'Chromium, which is not the pinned Chrome (not a pinned expectation).'
    );
    return undefined;
  }
  if (!existsSync(p)) {
    throw new Error(`PARITY_CHROME_PATH set but not found: ${p}`);
  }
  return p;
}

export function getDeterministicLaunchOptions({ requirePinned = false } = {}) {
  // Note: some flags may be ignored depending on platform/Chromium build.
  // We keep them because they reduce variance in practice.
  const args = [
    '--force-color-profile=srgb',
    '--disable-gpu-vsync',
    '--disable-features=RendererCodeIntegrity',
    '--disable-background-timer-throttling',
    '--disable-backgrounding-occluded-windows',
    '--disable-renderer-backgrounding',
    '--disable-lcd-text',
    '--disable-font-subpixel-positioning',
    '--disable-accelerated-2d-canvas',
    '--use-gl=swiftshader',
  ];

  return {
    headless: true,
    executablePath: getPinnedExecutablePath({ required: requirePinned }),
    args,
  };
}

export async function createDeterministicContext(browser, width, height, options = {}) {
  const { applyParityReset = false } = options;

  const context = await browser.newContext({
    viewport: { width, height },
    deviceScaleFactor: 1,
  });

  // Freeze time/animations as early as possible.
  if (existsSync(FREEZE_JS_PATH)) {
    await context.addInitScript({ path: FREEZE_JS_PATH });
  }

  // For micro-tests only: normalize default styles.
  if (applyParityReset && RESET_CSS) {
    await context.addInitScript({
      content: `(() => {
        const css = ${JSON.stringify(RESET_CSS)};
        const style = document.createElement('style');
        style.setAttribute('data-parity-reset', '1');
        style.textContent = css;
        document.documentElement.appendChild(style);
      })();`,
    });
  }

  return context;
}

export function shouldApplyParityResetForHtmlPath(htmlPath) {
  const p = String(htmlPath);
  // Micro-tests are designed to run under the parity reset.
  return p.includes('/websuite/micro/') || p.includes('\\websuite\\micro\\');
}
