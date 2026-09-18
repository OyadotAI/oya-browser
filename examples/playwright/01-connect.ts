// Bring your own tools: Playwright (or Puppeteer, Stagehand, browser-use) through Oya's CDP gateway.
import { chromium } from 'playwright-core';
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start({ provider: 'browserbase' });
const context = (await chromium.connectOverCDP(browser.cdpUrl!)).contexts()[0];
const page = context.pages()[0] ?? await context.newPage();
await page.goto('https://example.com');
console.log(await page.title());
