/** Drives the console against the QA stack and reports what each tab does. */
import { chromium } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';

const BASE = process.env.QA_URL || 'http://localhost:3200';
const KEY = process.env.QA_KEY || 'qa-key';
const OUT = process.env.QA_OUT || '/tmp/qa';
mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch({ channel: 'chrome' });
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
const errors = [];
page.on('console', (m) => m.type() === 'error' && errors.push(`console: ${m.text().slice(0, 300)}`));
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message.slice(0, 300)}`));
page.on('response', (r) => r.status() >= 400 && errors.push(`http ${r.status()} ${r.request().method()} ${r.url().replace(BASE, '')}`));
await page.addInitScript(([k]) => sessionStorage.setItem('oya_console_key', k), [KEY]);

await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
await page.waitForTimeout(2500);
await page.screenshot({ path: `${OUT}/00-dashboard.png`, fullPage: false });

const tabs = ['Browsers', 'Profiles', 'Playbooks', 'Control'];
const seen = [];
for (const name of tabs) {
  const tab = page.getByRole('button', { name, exact: true }).first();
  const count = await tab.count();
  if (!count) { seen.push(`${name}: NO TAB FOUND`); continue; }
  await tab.click();
  await page.waitForTimeout(1800);
  await page.screenshot({ path: `${OUT}/tab-${name}.png` });
  const text = (await page.locator('main, body').first().innerText()).replace(/\s+/g, ' ').slice(0, 400);
  seen.push(`${name}: ${text}`);
}
writeFileSync(`${OUT}/report.txt`, [...seen, '', '--- ERRORS ---', ...[...new Set(errors)]].join('\n'));
console.log(seen.join('\n----\n'));
console.log('\n--- ERRORS ---\n' + [...new Set(errors)].join('\n'));
await browser.close();
