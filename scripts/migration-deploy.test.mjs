/** Hosted rollout paths must apply the newest additive migration before their new server starts. */
import { readFileSync, readdirSync } from 'node:fs';
import { it } from 'node:test';
import assert from 'node:assert/strict';

/** The schema's latest numbered migration is the deployment contract, rather than a duplicated version literal. */
const latest = readdirSync(new URL('../server/migrations/', import.meta.url))
  .filter((name) => /^\d+_.*\.sql$/.test(name))
  .sort()
  .at(-1);

for (const file of ['deploy-dev.yaml', 'deploy-prod.yaml']) {
  it(`${file} applies the newest schema in every hosted rollout path`, () => {
    const workflow = readFileSync(new URL(`../.github/workflows/${file}`, import.meta.url), 'utf8');
    const commands = workflow
      .split('\n')
      .filter((line) => line.includes('psql ') && line.includes('--file=server/migrations/'));
    assert.ok(commands.length, 'hosted deployment must have a migration command');
    for (const command of commands)
      assert.ok(command.includes(`--file=server/migrations/${latest}`), `missing ${latest}`);
  });
}
