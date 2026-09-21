import { test, expect, type Page } from '@playwright/test';

test.use({ reducedMotion: 'reduce' });

async function dashboard(page: Page, cloudReady = false, signedIn = false) {
  let mode = 'agent';
  const commands: Array<{ action: string; params: Record<string, unknown> }> = [];
  const browser = {
    id: 'qa-browser',
    name: 'QA browser',
    clientType: 'oya',
    provider: 'oya-desktop',
    persona: null,
    personaName: null,
    health: 'ok',
    currentUrl: 'https://example.com',
    connectedAt: new Date().toISOString(),
    lastSeen: new Date().toISOString(),
    commands: 0,
    errors: 0,
    pending: 0,
    streaming: true,
    activity: [],
  };
  await page.context().addInitScript((account: boolean) => {
    // sessionStorage, not localStorage: the console keeps its credential for the
    // life of the tab, and the refresh token lives in an httpOnly cookie the
    // page cannot read. `oya_session` is the readable marker that says one
    // exists, which is what makes AuthProvider attempt a refresh at all.
    sessionStorage.setItem('oya_console_key', 'isolated-ui-test');
    if (account) document.cookie = 'oya_session=1; path=/';
    // A deterministic frame; no real browser session or credentials are used.
    const frame =
      'data:image/svg+xml,' +
      encodeURIComponent(
        '<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="600"><rect width="1000" height="600" fill="#eee"/><text x="40" y="60">Test browser</text></svg>',
      );
    class Frames {
      onmessage: ((e: { data: string }) => void) | null = null;
      timer = setInterval(() => this.onmessage?.({ data: frame }), 100);
      close() {
        clearInterval(this.timer);
      }
    }
    Object.defineProperty(window, 'EventSource', { value: Frames });
  }, signedIn);
  await page.context().route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname.replace(/^\/api/, '');
    let body: unknown = {};
    if (path.startsWith('/auth/')) body = { id: 'user-test', email: 'qa@example.com' };
    // An unsigned token with a far-future expiry; the mocks answer for it.
    if (path === '/auth/refresh')
      body = {
        access_token: `e30.${btoa(JSON.stringify({ exp: 4102444800 }))}.test`,
        user: { id: 'user-test', email: 'qa@example.com' },
        refresh_in_cookie: true,
      };
    // Metadata only. The server stores sha256(key), so there is no key to list.
    if (path === '/auth/keys')
      body = [
        { id: 'a'.repeat(64), prefix: 'isolated', project: 'prj-own', label: 'Checkout agents' },
        { id: 'b'.repeat(64), prefix: 'second-p', project: 'prj-two', label: 'Research' },
      ];
    if (path === '/auth/projects')
      body = [
        { id: 'prj-own', name: 'Checkout agents', role: 'administrator', owner: true },
        { id: 'prj-two', name: 'Research', role: 'administrator', owner: true },
        { id: 'prj-shared', name: 'Partner workspace', role: 'operator', owner: false },
      ];
    // Every project is opened the same way now, owned or shared.
    if (path === '/auth/projects/prj-own/access') body = { token: 'oya_own-credential' };
    if (path === '/auth/projects/prj-two/access') body = { token: 'oya_research-credential' };
    if (path === '/auth/projects/prj-shared/access') body = { token: 'oya_shared-credential' };
    if (path === '/control')
      body = {
        project: {
          id: 'prj-test',
          name: 'Test project',
          settings: { maxConcurrent: 3, budgetUsd: null, recordingDays: 7, auditDays: 90, rates: {}, policy: {} },
        },
        sessions: [{ ...browser, state: 'ready', managed: false, costUsd: 0, control: { mode } }],
        events: [],
        credentials: [],
        webhooks: [],
        deliveries: [],
      };
    if (path === '/control/members') body = { owner: null, members: [{ userId: 'member-test', role: 'operator' }] };
    if (path === '/control/sessions/qa-browser') body = { ...browser, state: 'ready', control: { mode } };
    if (path.endsWith('/ticket')) body = { ticket: 'one-use-test', expiresIn: 60 };
    if (path.endsWith('/control') && route.request().method() === 'POST') {
      mode = ({ acquire: 'human', release: 'paused', resume: 'agent' } as Record<string, string>)[
        route.request().postDataJSON().action
      ];
      body = { mode };
    }
    if (path === '/config')
      body = {
        onboarded: 'true',
        desktop_seen_at: 'now',
        browser_provider: 'oya-cloud',
        llm_provider: 'openai',
        chat_model: 'gpt-4o-mini',
        openai_api_key: '••••saved',
        effective: { model: 'gpt-4o-mini', baseUrl: 'https://api.openai.com/v1' },
        providers: [
          { id: 'oya-cloud', label: 'Oya Cloud', configured: cloudReady, needs: [] },
          { id: 'steel', label: 'Steel', configured: true, needs: ['steel_api_key'] },
        ],
      };
    if (path === '/providers')
      body = {
        providers: [
          { name: 'cdp', configured: true },
          { name: 'steel', configured: true },
          { name: 'browseruse', configured: false },
        ],
      };
    if (path === '/browsers') body = [browser];
    if (path === '/browsers/qa-browser') body = browser;
    if (path === '/personas')
      body = {
        personas: [
          {
            id: 'profile-qa',
            name: 'Research workspace',
            isDefault: false,
            createdAt: '2026-09-01T12:00:00Z',
            lastUsedAt: null,
            activeBrowsers: 1,
            maxConcurrent: 3,
            proxy: null,
            exit: null,
            prefs: null,
            fingerprint: { platform: 'MacIntel', timezone: 'America/Indiana/Indianapolis', screen: '1920 × 1080' },
            mfa: { configured: false },
            login: { sites: ['example.com', 'shop.example'], cookies: 8, updatedAt: null },
          },
        ],
      };
    if (path === '/fleet')
      body = {
        at: '2026-09-09T12:00:00Z',
        uptimeSeconds: 7200,
        sessions: { total: 0, attached: 0, recording: 0 },
        routing: { strategy: 'priority', queueDepth: 0, capacity: 10, active: 1, healthy: 1, providers: [] },
        usage: {
          hour: '2026-09-09T12:00:00Z',
          commands: 42,
          command_errors: 0,
          browser_seconds: 1800,
          browsers_started: 3,
        },
        limits: { commandsPerMinute: { limit: 100, burst: 20, remaining: 20 } },
        quotas: { chatTokensPerHour: 10000 },
        browsers: {
          total: 1,
          commands: 0,
          errors: 0,
          pending: 0,
          byClient: {},
          byProvider: {},
          byHealth: { ok: 1 },
          byPersona: {},
        },
      };
    if (path.endsWith('/command') || path.endsWith('/input')) {
      commands.push(route.request().postDataJSON());
      await new Promise((resolve) => setTimeout(resolve, 250));
      body = { ok: true };
    }
    await route.fulfill({ json: body });
  });
  await page.goto('/dashboard');
  await expect(page.getByRole('button', { name: /^Start browser/ }).first()).toBeVisible();
  return commands;
}

test('project actions rename, copy the API key, and delete the active project', async ({ page }) => {
  await dashboard(page, false, true);
  let projects = [
    { id: 'prj-own', name: 'Checkout agents', role: 'administrator', owner: true },
    { id: 'prj-two', name: 'Research', role: 'administrator', owner: true },
  ];
  await page.route('**/api/auth/projects', (route) => route.fulfill({ json: projects }));
  await page.route('**/api/auth/projects/prj-own', async (route) => {
    if (route.request().method() === 'PATCH') projects[0].name = route.request().postDataJSON().name;
    // Deleting stops the project's browsers; the confirmation says so.
    if (route.request().method() === 'DELETE') {
      expect(route.request().postDataJSON()).toEqual({ stopBrowsers: true });
      projects = projects.filter((p) => p.id !== 'prj-own');
    }
    await route.fulfill({ json: { ok: true } });
  });
  await page.route('**/api/auth/projects/prj-own/key', (route) =>
    route.fulfill({ json: { key: 'actual-project-api-key' } }),
  );
  await page.evaluate(() => {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async () => {
          throw new Error('Clipboard denied');
        },
      },
    });
  });
  await page.getByRole('button', { name: 'Switch project' }).click();
  await page.getByRole('button', { name: 'Options for Checkout agents', exact: true }).click();
  await page.getByRole('button', { name: 'Rename Checkout agents', exact: true }).click();
  await page.getByRole('textbox', { name: 'Project name' }).fill('Renamed agents');
  await page.getByRole('button', { name: 'Save name', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Rename Renamed agents', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Copy API key for Renamed agents', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'API key', exact: true })).toHaveValue('actual-project-api-key');
  await page.getByRole('button', { name: 'Back to projects' }).click();
  await page.getByRole('button', { name: 'Options for Renamed agents', exact: true }).click();
  await page.getByRole('button', { name: 'Delete Renamed agents', exact: true }).click();
  await page.getByRole('button', { name: 'Delete project', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Delete Renamed agents', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Switch project' })).toContainText('Research');
  expect(await page.evaluate(() => sessionStorage.getItem('oya_project_id'))).toBe('prj-two');
});

test('a project whose key cannot be read falls back and offers restore', async ({ page }) => {
  await dashboard(page, false, true);
  // Registered after the catch-all so it wins; reload so startup meets it with no project remembered.
  await page.context().route('**/api/auth/projects/prj-own/access', (route) =>
    route.fulfill({
      status: 503,
      json: { error: 'Project credentials could not be decrypted.', code: 'project_key_unavailable' },
    }),
  );
  await page.evaluate(() => sessionStorage.removeItem('oya_project_id'));
  await page.reload();
  const switcher = page.getByRole('button', { name: 'Switch project' });
  // The unreadable project is skipped at startup instead of leaving the console empty.
  await expect(switcher).toContainText('Research');
  await switcher.click();
  await page.getByRole('button', { name: /^Checkout agents/ }).click();
  await expect(page.locator('#project-picker').getByRole('alert')).toContainText('different server secret');
  await expect(switcher).toContainText('Research');
  await page.getByRole('button', { name: 'Restore with API key' }).click();
  await expect(page.getByRole('dialog', { name: 'Restore access' })).toBeVisible();
  await page.getByLabel('API key').fill('x'.repeat(32));
  await page.getByRole('button', { name: 'Restore access', exact: true }).click();
  // A key for some other project would add a second project, not repair this one.
  await expect(page.locator('#project-picker').getByRole('alert')).toContainText('doesn’t belong to Checkout agents');
});

test('one project switcher covers your projects and projects shared with you', async ({ page }) => {
  await dashboard(page, false, true);
  const switcher = page.getByRole('button', { name: 'Switch project' });
  await expect(switcher).toContainText('Checkout agents');
  await switcher.click();
  await expect(page.getByText('Your projects')).toBeVisible();
  await expect(page.getByText('Shared with you')).toBeVisible();
  await page.getByRole('button', { name: /^Partner workspace/ }).click();
  await expect(switcher).toContainText('Partner workspace');
  expect(await page.evaluate(() => sessionStorage.getItem('oya_project_credential'))).toBe('oya_shared-credential');
  await switcher.click();
  await page.getByRole('button', { name: /^Research/ }).click();
  await expect(switcher).toContainText('Research');
  // A project you own opens with a scoped credential too, no API key is
  // handed back by the server, and nothing lands on disk.
  expect(
    await page.evaluate(() => [sessionStorage.getItem('oya_project_credential'), localStorage.getItem('oya_api_key')]),
  ).toEqual(['oya_research-credential', null]);
  await switcher.click();
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByLabel('Project name')).toBeFocused();
});

test('project search, keyboard dismissal, and narrow screen layout', async ({ page }, testInfo) => {
  await dashboard(page, false, true);
  const trigger = page.getByRole('button', { name: 'Switch project' });
  await trigger.click();
  await page
    .getByRole('dialog', { name: 'Projects', exact: true })
    .screenshot({ path: testInfo.outputPath('projects-desktop.png') });
  await page.getByRole('textbox', { name: 'Search projects' }).fill('research');
  await expect(page.getByRole('button', { name: 'Options for Checkout agents' })).toHaveCount(0);
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('button', { name: 'Research', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(trigger).toContainText('Research');
  await trigger.click();
  await page.keyboard.press('Escape');
  await expect(trigger).toBeFocused();
  await expect(page.getByRole('dialog', { name: 'Projects', exact: true })).toHaveCount(0);
  await page.setViewportSize({ width: 375, height: 700 });
  await trigger.click();
  const panel = page.getByRole('dialog', { name: 'Projects', exact: true });
  const bounds = await panel.boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(375);
  await panel.screenshot({ path: testInfo.outputPath('projects-mobile.png') });
  await page.getByRole('button', { name: 'Options for Checkout agents' }).click();
  await page.getByRole('dialog').screenshot({ path: testInfo.outputPath('project-actions.png') });
});

test('dialog preserves text focus through fleet refreshes and restores its opener', async ({ page }) => {
  await dashboard(page, true);
  const opener = page.getByRole('button', { name: /^Start browser/ }).first();
  await opener.click();
  const name = page.getByLabel('Name (optional)');
  await name.fill('checkout');
  await page.waitForTimeout(3600); // Cross the actual 3-second fleet refresh.
  await expect(name).toBeFocused();
  await page.keyboard.type('-worker');
  await expect(name).toHaveValue('checkout-worker');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(opener).toBeFocused();
});

test('unavailable cloud default is explained before launching and a ready alternative can be selected', async ({
  page,
}) => {
  await dashboard(page);
  await page
    .getByRole('button', { name: /^Start browser/ })
    .first()
    .click();
  await expect(page.getByRole('button', { name: 'Start', exact: true })).toBeDisabled();
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('Cloud browsers');
  await page.getByLabel(/Provider/).selectOption('steel');
  await expect(page.getByRole('button', { name: 'Start', exact: true })).toBeEnabled();
});

test('live view consumes wheel events and preserves small trackpad deltas', async ({ page }) => {
  const commands = await dashboard(page);
  await page.getByText('QA browser', { exact: true }).click();
  await page.getByRole('button', { name: 'Take control', exact: true }).click();
  const live = page.getByLabel('Live view, click to control, Esc to release the keyboard');
  await expect(live.locator('img')).toBeVisible();
  await expect.poll(() => live.locator('img').evaluate((el: HTMLImageElement) => el.naturalWidth)).toBe(1000);
  const prevented = await live.evaluate((el) => {
    const rect = el.getBoundingClientRect();
    const event = new WheelEvent('wheel', {
      deltaY: 12,
      clientX: rect.x + 40,
      clientY: rect.y + 40,
      bubbles: true,
      cancelable: true,
    });
    el.dispatchEvent(event);
    return event.defaultPrevented;
  });
  expect(prevented).toBe(true);
  await expect.poll(() => commands.filter((c) => c.action === 'scroll').length).toBe(1);
  expect(commands.find((c) => c.action === 'scroll')?.params.amount).toBe(12);
});

test('live input completes each command before sending the next', async ({ page }) => {
  const commands = await dashboard(page);
  await page.getByText('QA browser', { exact: true }).click();
  await page.getByRole('button', { name: 'Take control', exact: true }).click();
  const live = page.getByLabel('Live view, click to control, Esc to release the keyboard');
  await expect.poll(() => live.locator('img').evaluate((el: HTMLImageElement) => el.naturalWidth)).toBe(1000);
  await live.click({ position: { x: 40, y: 40 } });
  await page.keyboard.type('abc');
  await page.keyboard.press('Enter');
  expect(commands.length).toBeLessThanOrEqual(1);
  await expect.poll(() => commands.length).toBe(3);
  expect(commands.map((c) => c.action)).toEqual(['click_coordinates', 'keyboard_type', 'press_key']);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('complementary', { name: 'Browser detail' })).toBeVisible();
});

test('dialog controls stay reachable on a short viewport and shortcuts stay inside the dialog', async ({ page }) => {
  await dashboard(page, true);
  await page.setViewportSize({ width: 800, height: 420 });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Settings' });
  const save = dialog.getByRole('button', { name: 'Save', exact: true });
  await expect(save).toBeInViewport();
  const scrolling = dialog.locator('.overflow-y-auto');
  expect(await scrolling.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
  await scrolling.hover();
  await page.mouse.wheel(0, 250);
  await expect.poll(() => scrolling.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
  await expect(save).toBeInViewport();
  await dialog.getByRole('button', { name: 'Close', exact: true }).focus();
  await page.keyboard.press('n');
  await expect(page.getByRole('dialog')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => document.body.style.overflow)).toBe('');
});

test('context-menu keyboard selection survives fleet refresh', async ({ page }) => {
  await dashboard(page, true);
  await page.getByText('QA browser', { exact: true }).click({ button: 'right' });
  await expect(page.getByRole('menu')).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.waitForTimeout(3600);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('dialog', { name: 'Connect to QA browser' })).toBeVisible();
});

test('settings discards cancelled drafts and saves a provider with its own credential', async ({ page }) => {
  await dashboard(page, true);
  const open = page.getByRole('button', { name: 'Settings', exact: true });
  await open.click();
  await page.getByLabel('Model', { exact: true }).selectOption('__custom');
  await page.getByLabel('Custom model ID').fill('custom-test');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await open.click();
  await expect(page.getByLabel('Model', { exact: true })).toHaveValue('gpt-4o-mini');
  await page.getByLabel('API key', { exact: true }).fill('fake-openai-draft');
  await page.getByRole('button', { name: /Claude/ }).click();
  await expect(page.getByLabel('API key', { exact: true })).toHaveValue('');
  const save = page.getByRole('button', { name: 'Save', exact: true });
  await expect(save).toBeDisabled();
  await page.getByLabel('API key', { exact: true }).fill('fake-claude-test-key');
  await page.getByLabel('Model', { exact: true }).selectOption('claude-haiku-4-5');
  const posted = page.waitForRequest((req) => req.url().endsWith('/api/config') && req.method() === 'POST');
  await save.click();
  expect((await posted).postDataJSON()).toMatchObject({
    llm_provider: 'anthropic',
    chat_model: 'claude-haiku-4-5',
    openai_api_key: 'fake-claude-test-key',
  });
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('code snippets have visible syntax colors and safely render masked keys', async ({ page }, testInfo) => {
  await dashboard(page, true);
  await page.getByText('QA browser', { exact: true }).click({ button: 'right' });
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Connect to QA browser' });
  await dialog.getByRole('tab', { name: 'TypeScript', exact: true }).click();
  const code = dialog.locator('code.syntax-code');
  await expect(code.locator('.token.keyword').first()).toHaveText('import');
  await expect(code).toContainText('<your-api-key>');
  expect(
    await code
      .locator('.token.keyword')
      .first()
      .evaluate((el) => getComputedStyle(el).color),
  ).not.toBe(await code.evaluate((el) => getComputedStyle(el).color));
  await page.waitForTimeout(250); // Let dialog and theme transitions settle for visual inspection.
  await page.screenshot({ path: testInfo.outputPath('snippets-dark.png') });
  await dialog.getByRole('tab', { name: 'Agents (MCP)', exact: true }).click();
  await expect(code).toHaveAttribute('data-language', 'json');
  expect(JSON.parse(await code.innerText()).mcpServers['qa-browser'].headers.Authorization).toBe(
    'Bearer <your-api-key>',
  );
  await dialog.getByRole('tab', { name: 'curl', exact: true }).click();
  await expect(code).toHaveAttribute('data-language', 'bash');
  await expect(code.locator('.token.string').first()).toBeVisible();
  await page.evaluate(() => (document.documentElement.dataset.theme = 'light'));
  await page.waitForTimeout(250); // Let dialog and theme transitions settle for visual inspection.
  await page.screenshot({ path: testInfo.outputPath('snippets-light.png') });
});

test('settings fits desktop and mobile with aligned controls', async ({ page }, testInfo) => {
  await dashboard(page, true);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByLabel('Model', { exact: true })).toHaveValue('gpt-4o-mini');
  await page.waitForTimeout(250); // Let dialog and theme transitions settle for visual inspection.
  await page.screenshot({ path: testInfo.outputPath('settings-desktop.png') });
  await page.setViewportSize({ width: 375, height: 812 });
  const dialog = page.getByRole('dialog', { name: 'Settings' });
  expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  await expect(dialog.getByRole('tab', { name: 'Verification' })).toBeInViewport();
  await expect(dialog.getByRole('button', { name: 'Save', exact: true })).toBeInViewport();
  await page.waitForTimeout(250); // Let dialog and theme transitions settle for visual inspection.
  await page.screenshot({ path: testInfo.outputPath('settings-mobile.png') });
  await page.evaluate(() => (document.documentElement.dataset.theme = 'light'));
  await page.waitForTimeout(250); // Let dialog and theme transitions settle for visual inspection.
  await page.screenshot({ path: testInfo.outputPath('settings-light.png') });
});

test('browser list contains long content and keeps actions reachable', async ({ page }, testInfo) => {
  await dashboard(page, true);
  await page.route('**/api/browsers', (route) =>
    route.fulfill({
      json: [
        {
          id: 'long-browser',
          name: 'Long workspace '.repeat(25),
          clientType: 'oya',
          provider: 'oya-cloud',
          persona: 'profile-qa',
          personaName: 'Operations '.repeat(25),
          health: 'ok',
          currentUrl: 'https://example.com/' + 'long-path/'.repeat(100),
          connectedAt: new Date().toISOString(),
          lastSeen: new Date().toISOString(),
          commands: 12345,
          errors: 0,
          pending: 0,
          streaming: false,
        },
      ],
    }),
  );
  await expect(
    page.getByRole('table', { name: 'Browsers' }).getByRole('button', { name: /^Long workspace/ }),
  ).toBeVisible();
  const table = page.getByRole('table', { name: 'Browsers' });
  expect(await table.evaluate((el) => el.getBoundingClientRect().width)).toBeLessThanOrEqual(1440);
  const row = table.locator('tbody tr').first();
  expect((await row.boundingBox())!.height).toBeLessThanOrEqual(70);
  await page.screenshot({ path: testInfo.outputPath('browsers-desktop.png') });
  await page.getByLabel('Filter browsers').fill('nothing matches this');
  await expect(page.getByText('No browsers match these filters.')).toBeVisible();
  await page.getByRole('button', { name: 'Clear filters', exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const scroll = page.locator('.data-scroll');
  await scroll.evaluate((el) => {
    el.scrollLeft = el.scrollWidth;
  });
  await expect(row.getByRole('button', { name: /^Stop / })).toBeInViewport();
  await page.screenshot({ path: testInfo.outputPath('browsers-mobile.png') });
});

test('profiles and control have bounded layouts and explain CDP sessions', async ({ page }, testInfo) => {
  await dashboard(page, true);
  await page.getByRole('button', { name: 'Profiles', exact: true }).click();
  const profiles = page.getByRole('table', { name: 'Profiles' });
  await expect(profiles).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('profiles-desktop.png') });
  await page.getByRole('button', { name: 'Control', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();
  await expect(page.getByText('Your browsers', { exact: true })).toBeVisible();
  await page.waitForTimeout(200);
  await page.screenshot({ path: testInfo.outputPath('control-desktop.png') });
  await page.getByRole('tab', { name: 'CDP sessions', exact: true }).click();
  await expect(page.getByText(/Individual REST or curl commands do not create a session/)).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('control-mobile.png') });
  await page.getByRole('button', { name: 'Profiles', exact: true }).click();
  await expect(profiles).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('profiles-mobile.png') });
});

test('adding providers preserves priority zero and clears credentials when switching vendor', async ({
  page,
}, testInfo) => {
  await dashboard(page);
  let submitted: Record<string, unknown> | undefined;
  await page.route('**/api/gateway/providers', async (route) => {
    submitted = route.request().postDataJSON();
    await route.fulfill({ json: { name: submitted?.name } });
  });
  await page.getByRole('button', { name: 'Control', exact: true }).click();
  await page.getByRole('tab', { name: 'Providers', exact: true }).click();
  await page.getByRole('button', { name: 'Add provider', exact: true }).click();
  const form = page.getByRole('form', { name: 'Add provider' });
  await form.getByLabel('Name', { exact: true }).fill('My browser');
  await form.getByLabel('Type', { exact: true }).selectOption('steel');
  await expect(form.getByLabel('Provider API key')).toHaveAttribute('placeholder', 'Use saved credential');
  await form.getByLabel('Provider API key').fill('steel-draft');
  await form.getByLabel('Type', { exact: true }).selectOption('browseruse');
  await expect(form.getByLabel('Provider API key')).toHaveValue('');
  await expect(form.getByLabel('Provider API key')).toHaveAttribute('required', '');
  await form.getByLabel('Provider API key').fill('browseruse-draft');
  await form.getByLabel('Priority (lower wins)').fill('0');
  await page.screenshot({ path: testInfo.outputPath('provider-form.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await form.getByRole('button', { name: 'Add provider', exact: true }).click();
  await expect(form).toHaveCount(0);
  expect(submitted).toMatchObject({
    name: 'My browser',
    type: 'browseruse',
    apiKey: 'browseruse-draft',
    priority: 0,
    maxConcurrent: 5,
  });
  await expect(page.getByRole('status')).toContainText('Provider saved');
});

test('provider errors remain visible across refresh and cancel clears the credential draft', async ({ page }) => {
  await dashboard(page);
  await page.route('**/api/gateway/providers', (route) =>
    route.fulfill({ status: 409, json: { error: 'A provider with this name already exists.' } }),
  );
  await page.getByRole('button', { name: 'Control', exact: true }).click();
  await page.getByRole('tab', { name: 'Providers', exact: true }).click();
  const add = page.getByRole('button', { name: 'Add provider', exact: true }).first();
  await add.click();
  const form = page.getByRole('form', { name: 'Add provider' });
  await form.getByLabel('Name', { exact: true }).fill('Duplicate');
  await form.getByLabel('Type', { exact: true }).selectOption('steel');
  await form.getByLabel('Provider API key').fill('draft-only');
  await form.getByRole('button', { name: 'Add provider', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'already exists' })).toBeVisible();
  await page.waitForTimeout(4500);
  await expect(page.getByRole('alert').filter({ hasText: 'already exists' })).toBeVisible();
  await form.getByRole('button', { name: 'Cancel', exact: true }).click();
  await add.click();
  await expect(form.getByLabel('Provider API key')).toHaveValue('');
});

test('opening a stream in a tab renders live frames and accepts browser input', async ({ page }, testInfo) => {
  const commands = await dashboard(page);
  await page.getByText('QA browser', { exact: true }).click();
  const stream = page.getByRole('link', { name: 'Stream', exact: true });
  await expect(stream).toHaveAttribute('href', '/live/qa-browser');
  const popupPromise = page.waitForEvent('popup');
  await stream.click();
  const viewer = await popupPromise;
  await expect(viewer.getByRole('heading', { name: 'QA browser' })).toBeVisible();
  await viewer.getByRole('button', { name: 'Take control', exact: true }).click();
  const live = viewer.getByLabel('Live view, click to control, Esc to release the keyboard');
  await expect.poll(() => live.locator('img').evaluate((el: HTMLImageElement) => el.naturalWidth)).toBe(1000);
  await viewer.screenshot({ path: testInfo.outputPath('live-tab.png') });
  await live.locator('img').click();
  await viewer.keyboard.type('hello');
  await expect.poll(() => commands.some((c) => c.action === 'keyboard_type')).toBe(true);
  expect(viewer.url()).not.toContain('key=');
  await viewer.close();
});

test('durable operations renders members, filters sessions, and fits a mobile viewport', async ({ page }, testInfo) => {
  await dashboard(page);
  await page.getByRole('button', { name: 'Control', exact: true }).click();
  await page.getByRole('tab', { name: 'Project operations', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Test project' })).toBeVisible();
  await expect(page.getByText('member-test · operator')).toBeVisible();
  await page.getByLabel('Filter sessions by state').selectOption('queued');
  await expect(page.getByText('No sessions match this view.')).toBeVisible();
  await page.getByLabel('Filter sessions by state').selectOption('ready');
  await expect(page.getByRole('button', { name: 'Take control', exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('operations-desktop.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('operations-mobile.png'), fullPage: true });
});

test('profiles manage proxies: add, check, and remove after confirming', async ({ page }) => {
  await dashboard(page);
  type Row = {
    id: string;
    label: string;
    kind: string;
    geo: string | null;
    shared: boolean;
    healthy: boolean;
    available: boolean;
    exitIp: string | null;
    lastCheckedAt: string | null;
    assigned: number;
    maxPersonas: number;
    cooldownMsRemaining: number;
  };
  let proxies: Row[] = [];
  let posted: Record<string, unknown> | null = null;
  await page.route('**/api/proxies**', async (route) => {
    const req = route.request();
    const path = new URL(req.url()).pathname;
    if (path.endsWith('/check')) {
      proxies = proxies.map((p) => ({ ...p, exitIp: '203.0.113.9', lastCheckedAt: new Date().toISOString() }));
      return route.fulfill({ json: { results: proxies.map((p) => ({ id: p.id, ok: true, exitIp: p.exitIp })) } });
    }
    if (req.method() === 'POST') {
      posted = req.postDataJSON();
      const row = {
        id: 'px-test',
        label: String(posted!.label),
        kind: String(posted!.kind),
        geo: String(posted!.geo),
        shared: false,
        healthy: true,
        available: true,
        exitIp: null,
        lastCheckedAt: null,
        assigned: 0,
        maxPersonas: Number(posted!.maxPersonas),
        cooldownMsRemaining: 0,
      };
      proxies.push(row);
      return route.fulfill({ status: 201, json: row });
    }
    if (req.method() === 'DELETE') {
      proxies = proxies.filter((p) => !path.endsWith(p.id));
      return route.fulfill({ json: { ok: true } });
    }
    return route.fulfill({ json: { proxies } });
  });

  await page.getByRole('button', { name: 'Profiles', exact: true }).click();
  await page.getByRole('button', { name: 'Proxies' }).click();
  const dialog = page.getByRole('dialog', { name: 'Proxies' });
  await expect(dialog.getByText('No proxies yet')).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Add proxy' })).toBeDisabled();

  await dialog.getByLabel('Proxy URL').fill('http://user:pass_session-a1@gate.example.com:7000');
  await dialog.getByLabel('Label').fill('us-home-1');
  await dialog.getByLabel('Country').fill('us');
  await dialog.getByRole('button', { name: 'Add proxy' }).click();
  const table = dialog.getByRole('table', { name: 'Proxies' });
  await expect(table.getByText('us-home-1')).toBeVisible();
  expect(posted).toMatchObject({
    url: 'http://user:pass_session-a1@gate.example.com:7000',
    label: 'us-home-1',
    geo: 'US',
    kind: 'residential',
    maxPersonas: 1,
  });
  await expect(dialog.getByLabel('Proxy URL')).toHaveValue('');

  await dialog.getByRole('button', { name: 'Check all' }).click();
  await expect(table.getByText('203.0.113.9')).toBeVisible();

  const remove = dialog.getByRole('button', { name: 'Remove us-home-1' });
  await remove.click();
  await expect(remove).toHaveText('Remove?');
  expect(proxies).toHaveLength(1);
  await remove.click();
  await expect(dialog.getByText('No proxies yet')).toBeVisible();
});

test('a mailbox second factor is configurable from the profile drawer', async ({ page }) => {
  await dashboard(page);
  let stored: Record<string, unknown> | null = null;
  await page.route('**/api/personas/profile-qa/mfa', async (route) => {
    stored = route.request().postDataJSON();
    return route.fulfill({ json: { configured: true, type: 'gmail', domain: 'portal.example.com' } });
  });

  await page.getByRole('button', { name: 'Profiles', exact: true }).click();
  await page.getByRole('button', { name: 'Research workspace' }).click();
  const drawer = page.getByRole('dialog', { name: 'Research workspace' });
  await drawer.getByLabel('MFA type').selectOption('gmail');
  await drawer.getByLabel('Refresh token').fill('rt-from-the-mailbox');
  // Store stays disabled until the mailbox has everything it needs to mint a token.
  await expect(drawer.getByRole('button', { name: 'Store factor' })).toBeDisabled();
  await drawer.getByLabel('OAuth client ID').fill('client-123.apps.googleusercontent.com');
  await drawer.getByLabel('Site this factor is for').fill('portal.example.com');
  await drawer.getByRole('button', { name: 'Store factor' }).click();
  await expect
    .poll(() => stored)
    .toEqual({
      domain: 'portal.example.com',
      type: 'gmail',
      refreshToken: 'rt-from-the-mailbox',
      clientId: 'client-123.apps.googleusercontent.com',
    });
  await expect(drawer.getByLabel('Refresh token')).toHaveValue('');
});

test('recording ignores a late poll, hands back control, and closes an active flow', async ({ page }) => {
  await dashboard(page);
  await page.route('**/api/playbooks', (route) => route.fulfill({ json: { playbooks: [] } }));
  let recording = false;
  let delayPoll = false;
  let releasePoll: (() => void) | undefined;
  const stops: Array<{ mode: string; resume?: boolean }> = [];
  await page.route('**/api/control/sessions/qa-browser/record', async (route) => {
    const request = route.request().postDataJSON();
    if (request.mode === 'start') recording = true;
    if (request.mode === 'stop') {
      recording = false;
      stops.push(request);
    }
    const response = {
      recording,
      steps: recording || stops.length ? [{ action: 'click', el: { text: 'Continue' } }] : [],
      secrets: [],
    };
    if (request.mode === 'status' && delayPoll) {
      delayPoll = false;
      await new Promise<void>((resolve) => {
        releasePoll = resolve;
      });
    }
    await route.fulfill({ json: response });
  });
  await page.getByRole('button', { name: 'Playbooks', exact: true }).click();
  await page.getByRole('button', { name: 'Record a flow', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Record a flow' });
  await dialog.getByRole('button', { name: 'Start recording', exact: true }).click();
  delayPoll = true;
  await expect.poll(() => !!releasePoll).toBe(true);
  await dialog.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(dialog.getByText('Stopped', { exact: true })).toBeVisible();
  releasePoll!();
  await expect(dialog.getByRole('button', { name: 'Save playbook', exact: true })).toBeVisible();
  expect(stops[0].resume).toBe(true);
  await dialog.getByRole('button', { name: 'Start new recording', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
  await dialog.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(dialog).not.toBeVisible();
  expect(stops).toHaveLength(2);
  expect(stops[1].resume).toBe(true);
});
