/** Replay validation must refuse unsafe or unusable destinations before running any deployment script. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseDeploymentPlan } from '../../../src/install/deployment-plan.ts';

describe('deployment plan', () => {
  it('keeps only supported deployment targets and option types', () => {
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'constructor', options: {} }), /deployment target/);
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'docker', options: { destroy: true } }), /option/);
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'ecs', options: { 'http-only': 'true' } }), /option/);
  });

  it('refuses credentials in saved options without echoing their values', () => {
    for (const options of [{ 'database-url': 'postgres://u:password@db/oya' }, { domain: 'https://u:password@host' }]) {
      assert.throws(
        () => parseDeploymentPlan({ version: 2, target: 'docker', options }),
        (err: Error) => {
          assert.ok(!err.message.includes('password'));
          return true;
        },
      );
    }
  });

  it('requires an explicit Kubernetes destination and one image source', () => {
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'k8s', options: {} }), /context/);
    const options = { context: 'cluster', registry: 'localhost:5001/oya', image: 'ghcr.io/acme/oya:tag' };
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'k8s', options }), /exactly one/);
    assert.throws(
      () => parseDeploymentPlan({ version: 2, target: 'k8s', options: { ...options, image: '', context: '../other' } }),
      /parent path/,
    );
  });

  it('does not silently downgrade ECS HTTPS or accept incomplete existing networking', () => {
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'ecs', options: {} }), /certificate/);
    assert.throws(
      () => parseDeploymentPlan({ version: 2, target: 'ecs', options: { 'certificate-arn': 'arn' } }),
      /publicUrl/,
    );
    assert.throws(
      () => parseDeploymentPlan({ version: 2, target: 'ecs', options: { 'http-only': true, 'vpc-id': 'vpc' } }),
      /subnets/,
    );
  });

  it('adds the examples defaults without accepting an implicit Google project', () => {
    assert.throws(() => parseDeploymentPlan({ version: 2, target: 'gcloud', options: {} }), /project/);
    const plan = parseDeploymentPlan({ version: 2, target: 'gcloud', options: { project: 'my-project' } });
    assert.deepEqual(plan.options, { project: 'my-project', region: 'us-central1', name: 'oya' });
  });

  it('rejects a malformed public URL without echoing credentials', () => {
    assert.throws(
      () =>
        parseDeploymentPlan({
          version: 2,
          target: 'ecs',
          publicUrl: 'https://u:password@',
          options: { 'certificate-arn': 'arn' },
        }),
      (err: Error) => {
        assert.ok(!err.message.includes('password'));
        return true;
      },
    );
  });
});
