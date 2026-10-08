/** Acceptance checks for the current public download, replay and documentation journeys. */
import { test, expect } from '@playwright/test';

test('landing is readable at desktop and mobile widths without changing the app theme', async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem('oya_theme', 'dark'));
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('The portal has no API.Your agent still gets in.');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(
    await page.locator('main').evaluate((el) => getComputedStyle(el.parentElement!.parentElement!).backgroundColor),
  ).toBe('rgb(255, 255, 255)');
  const cta = page.locator('section[aria-labelledby="hero-title"]').getByRole('link', { name: 'Start building' });
  await expect(cta).toBeInViewport();
  await expect(cta).toHaveAttribute('href', '/dashboard');
  await expect(page.getByRole('figure', { name: /saved playbook replaying/ })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('landing-desktop.png'), fullPage: true });
  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 844 });
    await page.evaluate(() => window.scrollTo(0, 0));
    await expect(cta).toBeInViewport();
    await expect(
      page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Open console' }),
    ).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath(`landing-mobile-${width}.png`), fullPage: true });
  }
  const textOf = (selector: string) =>
    page.locator(selector).evaluate((el) => {
      const copy = el.cloneNode(true) as HTMLElement;
      copy.querySelectorAll('pre').forEach((node) => node.remove());
      return copy.textContent ?? '';
    });
  // Above the fold is where a reader decides, and the landing pages worth copying spend
  // 45 to 150 words there. The hero gets a budget; the sections below it earn their space.
  expect((await textOf('section[aria-labelledby="hero-title"]')).trim().split(/\s+/).length).toBeLessThan(120);
  // Em-dashes are not used in this repo's copy: a comma, colon or full stop instead.
  expect(await textOf('main')).not.toContain('\u2014');
  await page.getByRole('link', { name: 'Read the docs' }).click();
  await expect(page.getByRole('heading', { level: 1 })).toContainText('A real browser for your agents.');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('portal example copies the displayed code and reports clipboard failures', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/');
  const example = page.locator('section[aria-labelledby="hero-title"]');
  await example.getByRole('button', { name: 'Copy example' }).focus();
  await page.keyboard.press('Enter');
  await expect(example.getByRole('status')).toHaveText('Example copied.');
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain('npm i @oya-ai/browser');
  expect(copied).toContain('await browser.play("eligibility-check", { memberId })');
  await page.evaluate(() => {
    Object.defineProperty(navigator.clipboard, 'writeText', {
      value: () => Promise.reject(new Error('Clipboard denied')),
    });
  });
  await example.getByRole('button', { name: 'Copy example' }).click();
  await expect(example.getByRole('status')).toContainText('Could not copy.');
  await expect(page.getByText('Select the code to copy it manually.', { exact: true })).toBeVisible();
  const footer = page.getByRole('navigation', { name: 'Footer navigation' });
  await expect(footer.getByRole('link', { name: 'SDK', exact: true })).toHaveAttribute(
    'href',
    'https://github.com/OyadotAI/oya-browser/tree/main/packages/sdk',
  );
  await expect(footer.getByRole('link', { name: 'Desktop' })).toHaveAttribute('href', '/docs#download');
  await expect(footer.getByRole('link', { name: 'Self-host' })).toHaveAttribute(
    'href',
    'https://github.com/OyadotAI/oya-browser/blob/main/docs/self-hosting.md',
  );
});

test('docs search and mobile navigation lead to readable sections', async ({ page }, testInfo) => {
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

test('download choices and walkthrough work without contacting external services', async ({ page }, testInfo) => {
  await page.route('https://www.loom.com/**', (route) =>
    route.fulfill({ contentType: 'text/html', body: '<p>Walkthrough fixture</p>' }),
  );
  await page.goto('/');
  const downloads = page.getByRole('navigation', { name: 'Browser downloads' });
  for (const [platform, extension] of [
    ['macOS', '.dmg'],
    ['Windows', '.exe'],
    ['Linux', '.AppImage'],
  ]) {
    await expect(downloads.getByRole('link', { name: `Download for ${platform}` })).toHaveAttribute(
      'href',
      new RegExp(`${extension}$`),
    );
  }
  await expect(page.locator('#how-it-works')).toContainText('Replay.');
  await expect(page.locator('iframe')).toHaveCount(0);
  await page.getByRole('button', { name: /Watch the walkthrough/ }).click();
  await expect(page.getByTitle('Oya Browser, faster scalable automation without CDP')).toHaveAttribute(
    'src',
    /loom.com\/embed\/.*autoplay=true/,
  );
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('walkthrough-mobile.png'), fullPage: true });
});
