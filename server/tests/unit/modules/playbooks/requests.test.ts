/**
 * Unit tests for checking playbook and run request bodies before any work
 * starts: save refusals, the run a recording becomes, and whether a named
 * playbook may play or run with the values given.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { saveRefusal, recordedRun, playable, runnable } = await import('../../../../src/modules/playbooks/requests.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { MAX_LEN } = await import('../../../../src/modules/playbooks/constants.ts');

const KEY = 'requests-key';
const PB = { name: 'signup', steps: [{ action: 'type', text: '{{email}} {{pw}}' }], defaults: { email: 'a@b' } };
const FILE = { file: 'a.pdf', type: 'application/pdf', b64: 'AAAA' };

describe('saveRefusal', () => {
  it('accepts a plain save and a version-2 one', () => {
    assert.equal(saveRefusal({} as any), null);
    assert.equal(saveRefusal({ schemaVersion: 2 } as any), null);
  });

  it('refuses an unsupported workflow version', () => {
    assert.equal(saveRefusal({ schemaVersion: 3 } as any), 'Unsupported workflow version');
  });

  it('refuses secrets that are not an array of variable names, when steps are sent', () => {
    const msg = 'secrets must be an array of variable names';
    assert.equal(saveRefusal({ steps: [], secrets: 'pw' } as any), msg);
    assert.equal(saveRefusal({ steps: [], secrets: ['ok', 'not ok'] } as any), msg);
    assert.equal(saveRefusal({ steps: [], secrets: ['pw'] } as any), null);
    assert.equal(saveRefusal({ secrets: 'ignored without steps' } as any), null);
  });
});

describe('recordedRun', () => {
  it('saves the browser’s last ask() when no steps are sent', () => {
    assert.equal(recordedRun({ name: 'x' }), undefined);
  });

  it('turns sanitized steps into a run named by its prompt, or its name', () => {
    const steps = [{ action: 'click', el: { text: 'Go' }, junk: 1 }];
    const run = recordedRun({ name: 'n', steps, secrets: [7] });
    assert.deepEqual(run, { prompt: 'n', steps: [{ action: 'click', el: { text: 'Go' } }], secrets: ['7'] });
    assert.equal(recordedRun({ prompt: 'x'.repeat(MAX_LEN + 1), steps }).prompt.length, MAX_LEN);
    assert.equal(recordedRun({ steps }).prompt, '');
  });

  it('refuses a recording with nothing to replay', () => {
    assert.throws(() => recordedRun({ steps: [] }), { status: 400 });
  });

  it('validates a version-2 draft and keeps its variables', () => {
    const run = recordedRun({
      schemaVersion: 2,
      steps: [{ action: 'navigate', url: 'https://a.test/{{q}}' }],
      variables: { q: { default: 'x' } },
      secrets: ['pw'],
    });
    assert.equal(run.schemaVersion, 2);
    assert.deepEqual(run.variables, { q: { secret: false, default: 'x' }, pw: { secret: true } });
  });
});

describe('playable and runnable', () => {
  beforeEach(async () => {
    keyConfig.reset();
    await keyConfig.savePlaybook(KEY, 'signup', PB);
  });

  it('plays a playbook when every variable without a default is given', () => {
    assert.deepEqual(playable(KEY, 'signup', { pw: 'x' }), { pb: { ...PB, name: 'signup' } });
    assert.deepEqual(playable(KEY, 'signup', { pw: FILE }).pb.name, 'signup');
  });

  it('refuses to play with a missing variable, an unknown playbook or bad values', () => {
    assert.deepEqual(playable(KEY, 'signup', {}), { status: 400, error: 'Missing variables: pw' });
    assert.deepEqual(playable(KEY, 'ghost', {}), { status: 404, error: 'No playbook named ghost' });
    assert.equal(playable(KEY, 'signup', { 'bad name': 'x' }).status, 400);
    assert.equal(playable(KEY, 'signup', [] as any).status, 400);
  });

  it('runs a prompt, or a playbook whose data and secrets cover it', () => {
    assert.deepEqual(runnable(KEY, { prompt: 'go', data: {}, secrets: {} } as any), { pb: null });
    assert.equal(runnable(KEY, { playbook: 'signup', data: {}, secrets: { pw: 'x' } } as any).pb.name, 'signup');
  });

  it('refuses both or neither of prompt and playbook', () => {
    const both = runnable(KEY, { prompt: 'go', playbook: 'signup', data: {}, secrets: {} } as any);
    assert.deepEqual(both, { status: 400, error: 'Pass exactly one of prompt or playbook' });
    assert.equal(runnable(KEY, { data: {}, secrets: {} } as any).status, 400);
  });

  it('refuses a file as a secret, and malformed data', () => {
    assert.equal(runnable(KEY, { prompt: 'go', data: {}, secrets: { f: FILE } } as any).status, 400);
    assert.equal(runnable(KEY, { prompt: 'go', data: { x: true }, secrets: {} } as any).status, 400);
  });

  it('refuses an unknown playbook and missing data', () => {
    assert.deepEqual(runnable(KEY, { playbook: 'ghost', data: {}, secrets: {} } as any), {
      status: 404,
      error: 'No playbook named ghost',
    });
    assert.deepEqual(runnable(KEY, { playbook: 'signup', data: {}, secrets: {} } as any), {
      status: 400,
      error: 'Missing data: pw',
    });
  });
});
