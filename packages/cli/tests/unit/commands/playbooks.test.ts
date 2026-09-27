/**
 * Unit tests for `oya playbooks` (src/commands/playbooks.ts): listing, and moving a
 * playbook between environments as an export file.
 */
import { FLAGS, captured, fakeFetch } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { cmdPlaybooks } from '../../../src/commands/playbooks.ts';

const DOC = { format: 'oya-playbook', version: 1, exportedAt: 'now', playbook: { name: 'signup', steps: [] } };
const dir = mkdtempSync(join(tmpdir(), 'oya-cli-playbooks-'));

describe('oya playbooks', () => {
  afterEach(() => mock.restoreAll());

  it('lists each playbook with its steps and inputs', async () => {
    fakeFetch({ 'GET /api/playbooks': { playbooks: [{ name: 'signup', steps: 3, variables: ['email'] }] } });
    const { out } = await captured(() => cmdPlaybooks([], FLAGS));
    assert.match(out, /signup {2}3 steps {2}email/);
  });

  it('export --out writes the export to a file', async () => {
    fakeFetch({ 'GET /api/playbooks/signup/export': DOC });
    const file = join(dir, 'signup.json');
    const { out } = await captured(() => cmdPlaybooks(['export', 'signup'], { ...FLAGS, out: file }));
    assert.deepEqual(JSON.parse(readFileSync(file, 'utf8')), DOC);
    assert.match(out, /signup written to/);
  });

  it('import sends the file under a new name, replacing only when asked', async () => {
    const calls = fakeFetch({ 'POST /api/playbooks/import': { name: 'signup-prod', steps: 3 } });
    const file = join(dir, 'in.json');
    writeFileSync(file, JSON.stringify(DOC));
    const { out } = await captured(() =>
      cmdPlaybooks(['import', file], { ...FLAGS, name: 'signup-prod', replace: true }),
    );
    assert.deepEqual(calls[0].body, { playbook: DOC, name: 'signup-prod', overwrite: true });
    assert.match(out, /imported signup-prod: 3 steps/);
  });

  it('refuses a subcommand it does not know, and an export or import with nothing named', async () => {
    fakeFetch({});
    await assert.rejects(cmdPlaybooks(['nope'], FLAGS), /Unknown playbooks subcommand "nope"/);
    await assert.rejects(cmdPlaybooks(['export'], FLAGS), /needs a name/);
    await assert.rejects(cmdPlaybooks(['import'], FLAGS), /needs a file/);
  });
});
