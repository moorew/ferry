import { chromium } from 'playwright';
const out = process.argv[2];
const base = 'http://127.0.0.1:4173';
const shots = [
  ['manager-history', '/?view=manager', 960, 640, async (p) => { await p.click('text=Quarterly review'); }],
  ['manager-settings', '/?view=manager', 960, 640, async (p) => { await p.click('button:has-text("Settings")'); }],
  ['firstrun', '/?view=manager&firstrun=1', 960, 640, async (p) => { await p.click('text=Continue'); }],
  ['picker', '/?view=picker', 640, 420, async (p) => { await p.keyboard.press('ArrowDown'); await p.keyboard.press('Tab'); }],
];
const browser = await chromium.launch({ executablePath: process.env.CHROME || undefined });
for (const scheme of ['light', 'dark'])
  for (const [name, path, w, h, act] of shots) {
    const page = await browser.newPage({ viewport: { width: w, height: h }, colorScheme: scheme });
    page.on('pageerror', (e) => console.log(name, 'pageerror', e.message));
    page.on('console', (m) => m.type() === 'error' && console.log(name, 'console', m.text()));
    await page.goto(base + path);
    await page.waitForTimeout(500);
    await act(page);
    await page.waitForTimeout(300);
    await page.screenshot({ path: `${out}/${name}-${scheme}.png` });
    await page.close();
  }
await browser.close();
console.log('done');
