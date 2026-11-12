/** Every setup value must explain its purpose, format, acquisition path and blank/default behavior. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { deploymentHelp } from '../../../src/install/deployment-help.ts';
import { DEPLOYMENT_TARGETS, type DeploymentTarget } from '../../../src/install/deployment-targets.ts';

describe('deployment guidance', () => {
  it('covers every accepted destination and Advanced option', () => {
    for (const [target, spec] of Object.entries(DEPLOYMENT_TARGETS)) {
      for (const field of Object.keys(spec.options)) {
        const help = deploymentHelp(target as DeploymentTarget, field);
        for (const key of ['title', 'purpose', 'find', 'behavior'] as const)
          assert.ok(help[key], `${target}.${field}.${key}`);
      }
    }
  });

  it('covers menus, public access and secret credentials as well as text fields', () => {
    for (const field of [
      'target',
      'image-source',
      'database-source',
      'access',
      'advanced',
      'public-url',
      'database-url',
    ])
      assert.ok(deploymentHelp('k8s', field).find);
    assert.match(deploymentHelp('k8s', 'database-url').behavior, /masked/);
  });

  it('quotes selected context names in displayed commands without executing them', () => {
    const help = deploymentHelp('k8s', 'namespace', { context: "team's cluster; $HOME" });
    assert.equal(help.command, "kubectl --context 'team'\\''s cluster; $HOME' get namespaces");
    assert.throws(() => deploymentHelp('k8s', 'unknown-field'), /Missing setup guidance/);
  });
});
