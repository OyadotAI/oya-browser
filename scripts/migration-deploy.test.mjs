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
    const lines = workflow.split('\n');
    const commands = lines.filter((line) => line.includes('migrate-in-cluster.sh "$NAMESPACE" app '));
    assert.ok(commands.length, 'hosted deployment must have a migration command');
    for (const command of commands) assert.ok(command.includes(`server/migrations/${latest}`), `missing ${latest}`);
  });

  it(`${file} migrates from inside the cluster, since a runner cannot reach a private-IP database`, () => {
    const workflow = readFileSync(new URL(`../.github/workflows/${file}`, import.meta.url), 'utf8');
    assert.ok(!/^\s*psql /m.test(workflow), 'runner-side psql would time out against Cloud SQL');
  });
}
