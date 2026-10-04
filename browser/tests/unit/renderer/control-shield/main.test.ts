/**
 * Unit test for the page's entry: it installs `window.oyaShield`, the one
 * function the main process calls, over the page's own document.
 */
import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { documentFrom } from '../../support/fake-dom.cjs';

const PAGE = fileURLToPath(new URL('../../../../src/renderer/control-shield/index.html', import.meta.url));

describe('the control shield entry', () => {
  const globals = globalThis as any;
  after(() => {
    delete globals.window;
    delete globals.document;
  });

  it('installs window.oyaShield, which plays an update on the page', async () => {
    const document = documentFrom(fs.readFileSync(PAGE, 'utf8'));
    const window: any = { innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1, requestAnimationFrame() {} };
    Object.assign(globals, { window, document });
    await import('../../../../src/renderer/control-shield/main.ts');
    window.oyaShield({ phase: 'scan' });
    assert.ok(document.body.classList.contains('scanning'));
  });
});
