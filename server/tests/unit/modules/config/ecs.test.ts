/**
 * Unit tests for the nested `ecs` setting: what it accepts and refuses, that a
 * masked credential sent back keeps the stored one, how it is read back, and
 * the environment names the ECS worker receives it as.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { validateEcs, viewEcs, ecsEnv } from '../../../../src/modules/config/ecs.ts';

/** A complete setting on an access key. */
const ECS = {
  cluster: 'browsers',
  taskDefinition: 'oya-browser:3',
  subnets: ['subnet-0abc'],
  region: 'us-east-1',
  auth: { type: 'iam', accessKeyId: 'AKIAABCDEFGHIJKLMNOP', secretAccessKey: 'shh' },
};

/** The 400 message for a value, or null when it is accepted. */
const refusal = (value, previous = null) => {
  try {
    validateEcs(value, previous);
    return null;
  } catch (err) {
    return err.message;
  }
};

describe('validateEcs', () => {
  it('accepts each kind of authentication', () => {
    assert.deepEqual(validateEcs(ECS), ECS);
    const role = { type: 'role', roleArn: 'arn:aws:iam::123456789012:role/oya-browsers' };
    assert.deepEqual(validateEcs({ ...ECS, auth: role }).auth, role);
    const sso = { type: 'sso', accessToken: 't', accountId: '123456789012', roleName: 'Browsers' };
    assert.deepEqual(validateEcs({ ...ECS, auth: sso }).auth, sso);
  });

  it('refuses a missing, unknown or malformed field, naming its path', () => {
    assert.match(refusal({ ...ECS, cluster: undefined })!, /^ecs\.cluster must be/);
    assert.match(refusal({ ...ECS, clutser: 'x' })!, /^ecs\.clutser must be one of: cluster/);
    assert.match(refusal({ ...ECS, subnets: [] })!, /^ecs\.subnets must be a non-empty list/);
    assert.match(refusal({ ...ECS, subnets: ['vpc-1'] })!, /^ecs\.subnets\[0\] must be subnet ids/);
    assert.match(refusal({ ...ECS, region: 'mars' })!, /^ecs\.region must be an AWS region/);
    assert.match(refusal({ ...ECS, assignPublicIp: 'yes' })!, /true or false/);
    assert.match(refusal('not an object')!, /^ecs must be an object/);
  });

  it('refuses an authentication it does not know, or one missing what it needs', () => {
    assert.match(refusal({ ...ECS, auth: { type: 'kerberos' } })!, /^ecs\.auth\.type must be one of: iam, role, sso/);
    assert.match(refusal({ ...ECS, auth: { type: 'role', roleArn: 'oya' } })!, /IAM role ARN/);
    assert.match(
      refusal({ ...ECS, auth: { type: 'sso', accessToken: 't', accountId: '1', roleName: 'r' } })!,
      /12-digit/,
    );
  });

  it('refuses an ExternalId, which is Oya’s, and an SSO profile, which is this host’s', () => {
    const role = { type: 'role', roleArn: 'arn:aws:iam::123456789012:role/r', externalId: 'mine' };
    assert.match(refusal({ ...ECS, auth: role })!, /^ecs\.auth\.externalId must be set by Oya/);
    assert.match(refusal({ ...ECS, auth: { type: 'sso', profile: 'ops' } })!, /^ecs\.auth\.profile must be one of/);
  });

  it('keeps the stored credential when its masked form is sent back', () => {
    const sent = { ...ECS, auth: { ...ECS.auth, secretAccessKey: '••••shh' } };
    assert.equal((validateEcs(sent, ECS) as any).auth.secretAccessKey, 'shh');
  });

  it('refuses a masked credential with nothing stored of that kind to keep', () => {
    const sent = { ...ECS, auth: { ...ECS.auth, secretAccessKey: '••••shh' } };
    assert.match(refusal(sent, null)!, /secretAccessKey/);
    assert.match(refusal(sent, { ...ECS, auth: { type: 'role', roleArn: 'x' } })!, /secretAccessKey/);
  });
});

describe('viewEcs and ecsEnv', () => {
  it('reads back with only the credentials masked', () => {
    const view: any = viewEcs(ECS, (v) => `••${v.length}`);
    assert.deepEqual(view.auth, { type: 'iam', accessKeyId: 'AKIAABCDEFGHIJKLMNOP', secretAccessKey: '••3' });
    assert.equal(view.cluster, 'browsers');
  });

  it('spells the setting as the environment the deployment would use', () => {
    const full = { ...ECS, securityGroups: ['sg-1', 'sg-2'], assignPublicIp: true, container: 'app' };
    assert.deepEqual(ecsEnv(full), {
      OYA_ECS_CLUSTER: 'browsers',
      OYA_ECS_TASK_DEFINITION: 'oya-browser:3',
      OYA_ECS_SUBNETS: 'subnet-0abc',
      OYA_ECS_SECURITY_GROUPS: 'sg-1,sg-2',
      OYA_ECS_ASSIGN_PUBLIC_IP: 'true',
      OYA_ECS_CONTAINER: 'app',
      AWS_REGION: 'us-east-1',
      OYA_ECS_AUTH: 'iam',
      AWS_ACCESS_KEY_ID: 'AKIAABCDEFGHIJKLMNOP',
      AWS_SECRET_ACCESS_KEY: 'shh',
    });
    const sso = { type: 'sso', accessToken: 't', accountId: '123456789012', roleName: 'r', ssoRegion: 'eu-west-1' };
    assert.equal(ecsEnv({ ...ECS, auth: sso }).OYA_ECS_SSO_REGION, 'eu-west-1');
    assert.equal(ecsEnv({ ...ECS, auth: { type: 'role', roleArn: 'arn' } }).OYA_ECS_ROLE_ARN, 'arn');
  });
});
