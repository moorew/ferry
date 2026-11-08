// Renders the phone app icons from design/assets/logo and the token colours
// (SPEC 21.5) into assets/icon/. Needs Playwright with Chromium:
//   node scripts/app-icons.mjs        (CHROMIUM=/path/to/chrome to choose a browser)
// The PNGs are committed; run this again only when the mark or colours change.

import { mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const design = join(root, '..', 'design');
const out = join(root, 'assets', 'icon');
mkdirSync(out, { recursive: true });

const tokens = JSON.parse(readFileSync(join(design, 'tokens', 'tokens.json'), 'utf8'));
const inner = (file) => {
  const svg = readFileSync(join(design, 'assets', 'logo', file), 'utf8');
  return svg.slice(svg.indexOf('>') + 1, svg.lastIndexOf('</svg>'));
};
// The mark's drawn extent inside its 64-unit box (the trail of dots hangs off the
// lower left), so the icon centres what you see rather than the box.
const MARK = { x: 1.5, y: 4.5, w: 55.5, h: 58.5 };

// `scale` is the mark's height as a share of the canvas.
function compose({ size, background, mark, scale, colour }) {
  const s = (size * scale) / MARK.h;
  const x = (size - MARK.w * s) / 2 - MARK.x * s;
  const y = (size - MARK.h * s) / 2 - MARK.y * s;
  const bg = background ? `<rect width="${size}" height="${size}" fill="${background}"/>` : '';
  const fill = colour ? ` fill="${colour}"` : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">${bg}<g transform="translate(${x} ${y}) scale(${s})"${fill}>${mark}</g></svg>`;
}

// A violet field with the one-colour mark in white (D-035). The system cuts the
// squircle on iOS and the adaptive shape on Android.
const violet = tokens.color.light.accent.$value;
const white = tokens.color.light['on-accent'].$value;
const mono = inner('mark-mono.svg');
const icons = {
  // iOS: full bleed. Dark keeps the violet field; tinted is a white mark the system tints.
  'ios-light.png': compose({ size: 1024, background: violet, mark: mono, scale: 0.56, colour: white }),
  'ios-dark.png': compose({ size: 1024, background: violet, mark: mono, scale: 0.56, colour: white }),
  'ios-tinted.png': compose({ size: 1024, background: null, mark: mono, scale: 0.56, colour: white }),
  // Android adaptive icon: the mark inside the 66/108 safe zone on the violet
  // background colour set in app.config.ts. The monochrome layer is for themed icons.
  'android-foreground.png': compose({ size: 1024, background: null, mark: mono, scale: 0.44, colour: white }),
  'android-monochrome.png': compose({ size: 1024, background: null, mark: mono, scale: 0.44, colour: '#000000' }),
};

// CHROMIUM picks a browser when the installed one doesn't match this Playwright.
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM || undefined });
const page = await browser.newPage({ viewport: { width: 1024, height: 1024 } });
for (const [name, svg] of Object.entries(icons)) {
  await page.setContent(`<html><body style="margin:0;background:transparent">${svg}</body></html>`);
  await page.locator('svg').screenshot({ path: join(out, name), omitBackground: true });
}
await browser.close();
console.log(`wrote ${Object.keys(icons).length} icons on ${violet}`);
