/** Acceptance checks for the current public download, replay and documentation journeys. */
import { test, expect } from '@playwright/test';

test('landing is readable at desktop and mobile widths without changing the app theme', async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem('oya_theme', 'dark'));
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Automate portal work. Keep control.');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await page.locator('main').evaluate((el) => getComputedStyle(el.parentElement!).backgroundColor)).toBe(
    'rgb(255, 255, 255)',
  );
  await expect(page.locator('link[href^="/assets/"], script[src^="/assets/"]')).toHaveCount(0);
  const cta = page.locator('section[aria-labelledby="hero-h"]').getByRole('link', { name: 'Book a demo' });
  await expect(cta).toBeInViewport();
  await expect(cta).toHaveAttribute('href', /calendly\.com/);
  await expect(page.getByRole('figure', { name: /Example replay:/ })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('landing-desktop.png'), fullPage: true });
  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 844 });
    await page.evaluate(() => window.scrollTo(0, 0));
    await expect(cta).toBeInViewport();
    await expect(
      page.locator('section[aria-labelledby="hero-h"]').getByRole('link', { name: 'Start building' }),
    ).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`landing-mobile-${width}.png`), fullPage: true });
  }
  const textOf = (selector: string) =>
    page.locator(selector).evaluate((el) => {
      const copy = el.cloneNode(true) as HTMLElement;
      copy.querySelectorAll('pre, figure, [hidden]').forEach((node) => node.remove());
      return copy.textContent ?? '';
    });
  // Above the fold is where a reader decides, and the landing pages worth copying spend
  // 45 to 150 words there. The hero gets a budget; the sections below it earn their space.
  expect((await textOf('section[aria-labelledby="hero-h"]')).trim().split(/\s+/).length).toBeLessThan(120);
  // Em-dashes are not used in this repo's copy: a comma, colon or full stop instead.
  expect(await textOf('main')).not.toContain('\u2014');
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.getByRole('navigation', { name: 'Main' }).getByRole('link', { name: /^Docs/ }).click();
  await expect(page.getByRole('heading', { level: 1 })).toContainText('A real browser for your agents.');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('the footer links the downloads and the docs', async ({ page }) => {
  await page.goto('/');
  const downloads = page.locator('#download');
  for (const platform of ['macOS', 'Windows']) {
    await expect(downloads.getByRole('link', { name: new RegExp(platform) })).toHaveAttribute('href', /\.(dmg|exe)$/);
  }
  await expect(page.getByRole('contentinfo').getByRole('link', { name: 'Docs' })).toHaveAttribute('href', '/docs');
});

test('docs search and mobile navigation lead to readable sections', async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem('oya_theme', 'dark'));
  await page.goto('/docs');
  await expect(page.getByRole('heading', { level: 1 })).toContainText('A real browser for your agents.');
  await page.screenshot({ path: testInfo.outputPath('docs-desktop.png') });
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await page.waitForTimeout(200); // Capture the settled theme, after color transitions.
  await page.screenshot({ path: testInfo.outputPath('docs-light.png') });
  await page.getByRole('button', { name: 'Switch to dark theme' }).click();
  const search = page.getByRole('textbox', { name: 'Search documentation' });
  await search.fill('MCP Setup');
  await page.getByRole('button', { name: 'MCP Setup', exact: true }).click();
  await expect(page.locator('#mcp-setup')).toBeInViewport();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.evaluate(() => window.scrollTo(0, 0));
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('docs-mobile.png') });
  await page.getByRole('button', { name: 'Open documentation menu' }).click();
  const dialog = page.getByRole('dialog', { name: 'Documentation' });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('link', { name: 'Quickstart', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('#quickstart')).toBeInViewport();
});

test('download choices work and the page fits a phone', async ({ page }, testInfo) => {
  await page.goto('/');
  const downloads = page.getByRole('navigation', { name: 'Browser downloads' });
  for (const [platform, extension] of [
    ['macOS', '.dmg'],
    ['Windows', '.exe'],
  ]) {
    await expect(downloads.getByRole('link', { name: new RegExp(platform) })).toHaveAttribute(
      'href',
      new RegExp(`${extension}$`),
    );
  }
  await expect(page.locator('#steps')).toContainText('Record once');
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('home-mobile.png'), fullPage: true });
});
