import { test, expect } from '@playwright/test';

test('landing is readable at desktop and mobile widths without changing the app theme', async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem('oya_theme', 'dark'));
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Run it once.Make it repeatable.');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(
    await page.locator('main').evaluate((el) => getComputedStyle(el.parentElement!.parentElement!).backgroundColor),
  ).toBe('rgb(255, 255, 255)');
  const cta = page.getByRole('link', { name: 'Start building' });
  await expect(cta).toBeInViewport();
  await expect(cta).toHaveAttribute('href', '/dashboard');
  const image = page.getByRole('img', { name: /Oya console showing three browsers/ });
  await expect(image).toBeVisible();
  await expect.poll(() => image.evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBeGreaterThan(0);
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
  const marketingCopy = await page.locator('main').evaluate((el) => {
    const copy = el.cloneNode(true) as HTMLElement;
    copy.querySelectorAll('pre').forEach((node) => node.remove());
    return copy.textContent ?? '';
  });
  expect(marketingCopy.trim().split(/\s+/).length).toBeLessThan(200);
  await page.getByRole('link', { name: 'Read the docs' }).click();
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Your browser,');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('portal example copies the displayed code and reports clipboard failures', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/');
  const displayed = await page.getByLabel('Portal automation TypeScript example').locator('code').innerText();
  expect(displayed.split('\n').length).toBeLessThanOrEqual(32);
  expect(displayed).toContain('await browser.toPlaybook(playbookName)');
  expect(displayed).toContain('await replay.play(playbookName,');
  expect(displayed).toContain('...secrets');
  await page.getByRole('button', { name: 'Copy example' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('status')).toHaveText('Example copied.');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(displayed);
  await page.evaluate(() => {
    Object.defineProperty(navigator.clipboard, 'writeText', {
      value: () => Promise.reject(new Error('Clipboard denied')),
    });
  });
  await page.getByRole('button', { name: 'Copy example' }).click();
  await expect(page.getByRole('status')).toContainText('Could not copy.');
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
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Your browser,');
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

test('playbook diagram explains recording, replay, and reviewed repairs', async ({ page }, testInfo) => {
  await page.goto('/');
  const record = page.getByRole('tab', { name: 'Record', exact: true });
  await expect(record).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tabpanel')).toContainText('Save the successful run');
  await record.focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', { name: 'Replay', exact: true })).toBeFocused();
  await expect(page.getByRole('tabpanel')).toContainText('Recorded steps replay without an LLM');
  await page.getByRole('tab', { name: 'Repair', exact: true }).click();
  await expect(page.getByRole('tabpanel')).toContainText('portal-request-review:draft');
  await expect(page.getByRole('tabpanel')).toContainText('Promote it after review');
  await page.screenshot({ path: testInfo.outputPath('landing-repair.png') });
  await page.keyboard.press('Home');
  await expect(record).toBeFocused();
  await expect(record).toHaveAttribute('aria-selected', 'true');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('tab', { name: 'Repair', exact: true }).click();
  await expect(page.getByRole('tabpanel')).toContainText('You review');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('landing-repair-mobile.png'), fullPage: true });
});
