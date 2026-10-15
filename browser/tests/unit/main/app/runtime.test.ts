/** Unit tests for src/main/app/runtime.ts: only a person's own desktop counts as the desktop. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { desktopRuntime } from '../../../../src/main/app/runtime.ts';

it('counts only a person’s own desktop as the desktop: no provider, not in Docker', () => {
  assert.equal(desktopRuntime({ values: {} } as any, {}), true);
  assert.equal(desktopRuntime({ values: { provider: 'oya-cloud' } } as any, {}), false);
  assert.equal(desktopRuntime({ values: {} } as any, { OYA_DOCKER: '1' }), false);
});
