// Bring your own tools: Playwright (or Puppeteer, Stagehand, browser-use) through Oya's CDP gateway.
import { chromium } from 'playwright-core';
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start();
const cdp = await chromium.connectOverCDP(browser.cdpUrl!);
const context = cdp.contexts()[0];
const page = context.pages()[0] ?? (await context.newPage());
await page.goto('https://example.com');
console.log(await page.title());
await cdp.close(); // drops Playwright's connection; without it Node waits on the socket of a browser stop() leaves running
