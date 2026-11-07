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
const MARK_BOX = 64;

// `scale` is the mark's share of the canvas; the mark is centred on its own box.
function compose({ size, background, mark, scale, colour }) {
  const s = (size * scale) / MARK_BOX;
  const offset = (size - MARK_BOX * s) / 2;
  const bg = background ? `<rect width="${size}" height="${size}" fill="${background}"/>` : '';
  const fill = colour ? ` fill="${colour}"` : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">${bg}<g transform="translate(${offset} ${offset}) scale(${s})"${fill}>${mark}</g></svg>`;
}

const light = tokens.color.light.bg.$value;
const dark = tokens.color.dark.bg.$value;
const icons = {
  // iOS: full bleed, the system rounds the corners. Light, dark and tinted (iOS 18+).
  'ios-light.png': compose({ size: 1024, background: light, mark: inner('mark.svg'), scale: 0.62 }),
  'ios-dark.png': compose({ size: 1024, background: null, mark: inner('mark-dark.svg'), scale: 0.62 }),
  'ios-tinted.png': compose({ size: 1024, background: null, mark: inner('mark-mono.svg'), scale: 0.62, colour: '#FFFFFF' }),
  // Android adaptive icon: the mark inside the 66/108 safe zone; the background is
  // a colour in app.config.ts. The monochrome layer is for themed icons.
  'android-foreground.png': compose({ size: 1024, background: null, mark: inner('mark.svg'), scale: 0.5 }),
  'android-monochrome.png': compose({ size: 1024, background: null, mark: inner('mark-mono.svg'), scale: 0.5, colour: '#000000' }),
};

// CHROMIUM picks a browser when the installed one doesn't match this Playwright.
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM || undefined });
const page = await browser.newPage({ viewport: { width: 1024, height: 1024 } });
for (const [name, svg] of Object.entries(icons)) {
  await page.setContent(`<html><body style="margin:0;background:transparent">${svg}</body></html>`);
  await page.locator('svg').screenshot({ path: join(out, name), omitBackground: true });
}
await browser.close();
console.log(`wrote ${Object.keys(icons).length} icons; Android background ${light}, iOS dark ground ${dark} is the system's`);
