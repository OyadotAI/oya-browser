/** Production notification gating is checked against the real workflow without a YAML dependency. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

/** Read the shipped workflow so a dependency or condition regression fails the normal script suite. */
const workflow = readFileSync(new URL('../.github/workflows/deploy-prod.yaml', import.meta.url), 'utf8');

/** Isolates one job using the workflow's existing two-space job indentation. */
function job(name) {
  return workflow.match(new RegExp(`^  ${name}:[\\s\\S]*?(?=^  [\\w-]+:|(?![\\s\\S]))`, 'm'))?.[0];
}

/** Exercise the actual Actions condition, including the path whose other dependency is skipped. */
const notification = job('indexnow');
const condition = notification.match(/^    if: \$\{\{ (.*) \}\}$/m)[1];
const results = ['success', 'failure', 'skipped', 'cancelled'];

test('only a successful, uncancelled production deployment triggers IndexNow', () => {
  for (const deploy of results)
    for (const server of results)
      for (const cancelled of [false, true]) {
        const needs = { deploy: { result: deploy }, 'server-only': { result: server } };
        const actual = runInNewContext(condition, { needs, cancelled: () => cancelled });
        assert.equal(
          actual,
          !cancelled && (deploy === 'success' || server === 'success'),
          JSON.stringify({ needs, cancelled }),
        );
      }
  assert.match(notification, /needs: \[deploy, server-only\]/);
  assert.match(condition, /!cancelled\(\)/);
  assert.doesNotMatch(workflow.split('jobs:')[0], /pull_request/);
});

test('notification is bounded, nonblocking and reports failures including setup failures', () => {
  assert.match(notification, /timeout-minutes: 5/);
  assert.match(notification, /continue-on-error: true/);
  assert.match(notification, /permissions:\n      contents: read\n    steps:/);
  assert.match(notification, /- name: Report notification failure\n      if: failure\(\)/);
  assert.match(
    notification,
    /::warning title=IndexNow notification failed::Production deployment succeeded; the IndexNow notification failed\./,
  );
  assert.match(notification, /Retry from the repository root: `npm run indexnow`\./);
});

test('notification uses pinned actions and the production command once without installing dependencies', () => {
  const actions = [...notification.matchAll(/uses: (\S+)/g)].map((match) => match[1]);
  assert.equal(actions.length, 2);
  for (const action of actions) assert.match(action, /^actions\/(checkout|setup-node)@[a-f0-9]{40}$/);
  assert.match(notification, /persist-credentials: false/);
  assert.match(notification, /node-version: 24/);
  assert.match(notification, /SITE_URL: https:\/\/oyabrowser\.com\n      run: npm run indexnow/);
  assert.equal([...notification.matchAll(/run: npm run indexnow/g)].length, 1);
  assert.doesNotMatch(notification, /npm (ci|install)|secrets\./);
});

test('deployments and package publishing remain independent of notification', () => {
  const rest = workflow.replace(notification, '');
  assert.doesNotMatch(rest, /^    needs:.*indexnow/m);
  assert.match(job('publish-npm'), /needs: deploy\n/);
  assert.match(job('publish-pypi'), /needs: build-pypi\n/);
  assert.match(job('build-pypi'), /needs: deploy\n/);
});
