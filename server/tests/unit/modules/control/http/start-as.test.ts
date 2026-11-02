/**
 * Unit tests for what a browser start runs as (control/http/start-as.ts): one
 * persona, or a group's members, and the request shapes that are refused.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { startAs } from '../../../../../src/modules/control/http/start-as.ts';

/** Fakes that record what they were asked. */
function deps() {
  return {
    personas: { resolve: mock.fn((_key: string, persona?: string | null) => ({ id: persona || 'default' })) },
    groups: { choices: mock.fn(async (_key: string, name: string) => [{ persona: `${name}-1`, personaLimit: 1 }]) },
  };
}

describe('startAs', () => {
  it('resolves a persona as before when no group is named', async () => {
    const d = deps();
    assert.deepEqual(await startAs(d, 'k', { persona: 'p1' }), { persona: { id: 'p1' } });
    assert.deepEqual(await startAs(d, 'k', { profile: 'p2' }), { persona: { id: 'p2' } });
    assert.deepEqual(await startAs(d, 'k', undefined), { persona: { id: 'default' } });
  });

  it('lists the members of a group named personaGroup or profileGroup', async () => {
    const d = deps();
    assert.deepEqual(await startAs(d, 'k', { personaGroup: 'pool' }), {
      personaChoices: [{ persona: 'pool-1', personaLimit: 1 }],
    });
    await startAs(d, 'k', { profileGroup: 'other', personaGroup: 'other' });
    assert.deepEqual(d.groups.choices.mock.calls[1].arguments, ['k', 'other']);
    assert.equal(d.personas.resolve.mock.callCount(), 0);
  });

  it('refuses a group with a persona, two different group names, and a name that is not text', async () => {
    const d = deps();
    await assert.rejects(startAs(d, 'k', { personaGroup: 'pool', persona: 'p1' }), {
      status: 400,
      code: 'invalid_request',
      message: 'Pass personaGroup or persona, not both',
    });
    await assert.rejects(startAs(d, 'k', { personaGroup: 'a', profileGroup: 'b' }), {
      message: 'Pass personaGroup or profileGroup, not both',
    });
    await assert.rejects(startAs(d, 'k', { personaGroup: 7 }), { status: 400 });
    await assert.rejects(startAs(d, 'k', { personaGroup: '' }), { status: 400 });
  });
});
