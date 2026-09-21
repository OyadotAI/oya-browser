/**
 * Unit tests for a key's saved playbooks: saving a run by name (its values
 * templated), promoting a healed draft, renaming with the draft, listing
 * newest first, and deleting.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const catalog = await import('../../../../src/modules/playbooks/catalog.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const recorder = await import('../../../../src/modules/agent/recorder.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const KEY = 'catalog-key';
const RUN = {
  prompt: 'Sign up ada@x.test',
  steps: [
    { action: 'navigate', url: 'https://a.test/', start: true },
    { action: 'type', text: 'ada@x.test', el: { name: 'email' } },
    { action: 'click', el: { text: 'Go' } },
  ],
  secrets: ['pw'],
};

describe('playbook catalog', () => {
  beforeEach(() => keyConfig.reset());
  afterEach(() => mock.timers.reset());

  describe('create', () => {
    it('saves a run with its typed values templated, and describes it', async () => {
      const described = await catalog.create(KEY, 'b-1', 'signup', structuredClone(RUN));
      assert.deepEqual(described.variables, ['email']);
      assert.deepEqual(described.defaults, { email: 'ada@x.test' });
      assert.equal(described.steps, 3);
      assert.match(described.code, /export default async function run/);
      const stored = keyConfig.getPlaybook(KEY, 'signup');
      assert.equal(stored.prompt, 'Sign up {{email}}');
      assert.deepEqual(stored.secrets, ['pw']);
      assert.ok(stored.createdAt);
    });

    it('refuses a name that is not 1-64 letters, digits, _ or -', async () => {
      for (const name of ['', 'a b', 'x'.repeat(65), 'a:draft', 42]) {
        await assert.rejects(catalog.create(KEY, 'b-1', name, RUN), { status: 400 }, String(name));
      }
    });

    it('refuses a run that only navigates, or no run at all', async () => {
      await assert.rejects(catalog.create(KEY, 'b-1', 'x', { steps: [RUN.steps[0]] }), {
        status: 409,
        message: /only visited pages/,
      });
      await assert.rejects(catalog.create(KEY, 'b-never', 'x'), {
        status: 409,
        message: /run ask\(\) on this browser first/,
      });
    });

    it('saves the browser’s last ask() when no run is passed', async () => {
      const browser = scriptedBrowser('b-catalog');
      await recorder.startRun('b-catalog', { prompt: 'p', steps: [], elements: [], secrets: [] });
      recorder.recordStep('b-catalog', 'press_key', { key: 'Enter' }, {});
      await catalog.create(KEY, 'b-catalog', 'enter');
      assert.deepEqual(keyConfig.getPlaybook(KEY, 'enter').steps, [{ action: 'press_key', key: 'Enter' }]);
      browser.disconnect();
    });

    it('keeps a workflow run’s declared variables, with non-secret defaults', async () => {
      const run = {
        schemaVersion: 2,
        prompt: 'wf',
        steps: [
          { action: 'navigate', url: 'https://a.test/{{q}}' },
          { action: 'click', el: { text: 'Go' } },
        ],
        variables: { q: { default: 'x' }, pw: { secret: true } },
      };
      const described = await catalog.create(KEY, 'b-1', 'wf', run);
      assert.deepEqual(described.defaults, { q: 'x' });
      assert.equal(keyConfig.getPlaybook(KEY, 'wf').schemaVersion, 2);
    });
  });

  describe('promote', () => {
    it('makes the healed draft the playbook and removes the draft', async () => {
      await catalog.create(KEY, 'b-1', 'signup', structuredClone(RUN));
      const draft = { ...keyConfig.getPlaybook(KEY, 'signup'), steps: [RUN.steps[2]], healedAt: 'x', healedFrom: 1 };
      await keyConfig.savePlaybook(KEY, 'signup:draft', draft);
      const described = await catalog.promote(KEY, 'signup');
      assert.equal(described.steps, 1);
      const pb = keyConfig.getPlaybook(KEY, 'signup');
      assert.ok(pb.promotedAt);
      assert.equal(pb.healedAt, undefined);
      assert.equal(keyConfig.getPlaybook(KEY, 'signup:draft'), null);
    });

    it('answers 404 when there is no draft', async () => {
      await assert.rejects(catalog.promote(KEY, 'nothing'), { status: 404, message: 'No healed draft for nothing' });
    });
  });

  describe('rename', () => {
    it('moves the playbook and its draft to the new name', async () => {
      await catalog.create(KEY, 'b-1', 'old', structuredClone(RUN));
      await keyConfig.savePlaybook(KEY, 'old:draft', { steps: [], name: 'old:draft' });
      const described = await catalog.rename(KEY, 'old', 'new');
      assert.equal(described.name, 'new');
      assert.equal(keyConfig.getPlaybook(KEY, 'old'), null);
      assert.equal(keyConfig.getPlaybook(KEY, 'old:draft'), null);
      assert.equal(keyConfig.getPlaybook(KEY, 'new:draft').name, 'new:draft');
    });

    it('renames a playbook without a draft', async () => {
      await catalog.create(KEY, 'b-1', 'old', structuredClone(RUN));
      await catalog.rename(KEY, 'old', 'new');
      assert.equal(keyConfig.getPlaybook(KEY, 'new:draft'), null);
      assert.ok(keyConfig.getPlaybook(KEY, 'new'));
    });

    it('answers with the playbook unchanged when the name is the same', async () => {
      await catalog.create(KEY, 'b-1', 'same', structuredClone(RUN));
      assert.equal((await catalog.rename(KEY, 'same', 'same')).name, 'same');
    });

    it('refuses a missing playbook, a draft, a taken name and an invalid one', async () => {
      await catalog.create(KEY, 'b-1', 'a', structuredClone(RUN));
      await catalog.create(KEY, 'b-1', 'b', structuredClone(RUN));
      await keyConfig.savePlaybook(KEY, 'a:draft', { steps: [] });
      await assert.rejects(catalog.rename(KEY, 'ghost', 'c'), { status: 404 });
      await assert.rejects(catalog.rename(KEY, 'a:draft', 'c'), { status: 404 });
      await assert.rejects(catalog.rename(KEY, 'a', 'b'), {
        status: 409,
        message: 'A playbook named b already exists',
      });
      await assert.rejects(catalog.rename(KEY, 'a', 'bad name'), { status: 400 });
    });
  });

  describe('list', () => {
    it('lists playbooks newest first, each with its waiting draft', async () => {
      mock.timers.enable({ apis: ['Date'], now: 1_000 });
      await catalog.create(KEY, 'b-1', 'older', structuredClone(RUN));
      mock.timers.tick(1_000);
      await catalog.create(KEY, 'b-1', 'newer', structuredClone(RUN));
      await keyConfig.savePlaybook(KEY, 'older:draft', { steps: [], healedAt: 'h', healedFrom: 2 });
      const listed = catalog.list(KEY);
      assert.deepEqual(
        listed.map((p) => p.name),
        ['newer', 'older'],
      );
      assert.equal(listed[0].draft, null);
      assert.deepEqual([listed[1].draft.healedAt, listed[1].draft.healedFrom], ['h', 2]);
      assert.equal(listed[0].promotedAt, null);
    });

    it('lists nothing for a key with no playbooks', () => {
      assert.deepEqual(catalog.list('empty-key'), []);
    });
  });

  describe('remove', () => {
    it('deletes a playbook and its draft', async () => {
      await catalog.create(KEY, 'b-1', 'gone', structuredClone(RUN));
      await keyConfig.savePlaybook(KEY, 'gone:draft', { steps: [] });
      await catalog.remove(KEY, 'gone');
      assert.deepEqual(keyConfig.listPlaybooks(KEY), []);
    });

    it('deletes just a draft when named', async () => {
      await catalog.create(KEY, 'b-1', 'kept', structuredClone(RUN));
      await keyConfig.savePlaybook(KEY, 'kept:draft', { steps: [] });
      await catalog.remove(KEY, 'kept:draft');
      assert.ok(keyConfig.getPlaybook(KEY, 'kept'));
      assert.equal(keyConfig.getPlaybook(KEY, 'kept:draft'), null);
    });

    it('answers 404 for a playbook that does not exist', async () => {
      await assert.rejects(catalog.remove(KEY, 'ghost'), { status: 404, message: 'No playbook named ghost' });
    });
  });
});
