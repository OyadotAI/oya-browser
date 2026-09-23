/**
 * Unit tests for how the ECS worker signs in to AWS: the default chain, an
 * access key, an assumed role and IAM Identity Center (the deployment's
 * profile, or a key's own access token), and which of them is a key's own
 * account. No AWS call is made; the SSO exchange is stubbed.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { SSOClient } from '@aws-sdk/client-sso';
import { Status } from '../../../../../src/platform/http-status.ts';
import {
  AUTH_TYPES,
  authFor,
  authMissing,
  credentialsFor,
  ownAccount,
} from '../../../../../src/drivers/sandbox/workers/ecs-auth.ts';

/** A key's own SSO sign-in. */
const SSO = {
  OYA_ECS_AUTH: 'sso',
  OYA_ECS_SSO_ACCESS_TOKEN: 'token',
  OYA_ECS_SSO_ACCOUNT_ID: '123456789012',
  OYA_ECS_SSO_ROLE_NAME: 'Browsers',
  AWS_REGION: 'us-east-1',
};

afterEach(() => mock.restoreAll());

describe('authFor and authMissing', () => {
  it('uses the default chain when nothing is chosen', () => {
    assert.deepEqual(authFor({}), { type: 'default' });
    assert.deepEqual(authMissing({}), []);
  });

  it('reads each kind and names what it lacks', () => {
    assert.deepEqual(authMissing({ OYA_ECS_AUTH: 'iam' }), ['AWS_ACCESS_KEY_ID', 'AWS_SECRET_ACCESS_KEY']);
    assert.deepEqual(authFor({ OYA_ECS_AUTH: 'role', OYA_ECS_ROLE_ARN: 'arn' }), {
      type: 'role',
      roleArn: 'arn',
      externalId: null,
    });
    assert.deepEqual(authMissing({ OYA_ECS_AUTH: 'sso' }), ['OYA_ECS_SSO_PROFILE']);
    assert.deepEqual(authMissing({ OYA_ECS_AUTH: 'sso', OYA_ECS_SSO_ACCESS_TOKEN: 't' }), [
      'OYA_ECS_SSO_ACCOUNT_ID',
      'OYA_ECS_SSO_ROLE_NAME',
    ]);
    assert.equal(authFor(SSO)!.ssoRegion, 'us-east-1', 'the SSO portal defaults to the ECS region');
  });

  it('refuses an authentication type that does not exist, naming those that do', () => {
    assert.equal(authFor({ OYA_ECS_AUTH: 'kerberos' }), null);
    assert.match(authMissing({ OYA_ECS_AUTH: 'kerberos' })[0], new RegExp(AUTH_TYPES.join(', ')));
  });
});

describe('ownAccount', () => {
  it('counts an access key, a role or an SSO token as a key’s own account', () => {
    assert.equal(ownAccount({ OYA_ECS_AUTH: 'iam', AWS_ACCESS_KEY_ID: 'a', AWS_SECRET_ACCESS_KEY: 's' }), true);
    assert.equal(ownAccount({ OYA_ECS_AUTH: 'role', OYA_ECS_ROLE_ARN: 'arn' }), true);
    assert.equal(ownAccount(SSO), true);
  });

  it('never counts the default chain, half a key or an SSO profile, which are this host’s', () => {
    assert.equal(ownAccount({}), false);
    assert.equal(ownAccount({ OYA_ECS_AUTH: 'iam', AWS_ACCESS_KEY_ID: 'a' }), false);
    assert.equal(ownAccount({ OYA_ECS_AUTH: 'sso', OYA_ECS_SSO_PROFILE: 'ops' }), false);
    assert.equal(ownAccount({ OYA_ECS_AUTH: 'kerberos' }), false);
  });
});

describe('credentialsFor', () => {
  it('leaves the default chain to the SDK and passes an access key as is', async () => {
    assert.equal(await credentialsFor({ type: 'default' }, 'us-east-1'), undefined);
    const key = authFor({
      OYA_ECS_AUTH: 'iam',
      AWS_ACCESS_KEY_ID: 'a',
      AWS_SECRET_ACCESS_KEY: 's',
      AWS_SESSION_TOKEN: 't',
    });
    assert.deepEqual(await credentialsFor(key, 'us-east-1'), {
      accessKeyId: 'a',
      secretAccessKey: 's',
      sessionToken: 't',
    });
  });

  it('makes refreshing providers for a role and for an SSO profile', async () => {
    const role = authFor({
      OYA_ECS_AUTH: 'role',
      OYA_ECS_ROLE_ARN: 'arn:aws:iam::1:role/r',
      OYA_ECS_EXTERNAL_ID: 'oya-x',
    });
    assert.equal(typeof (await credentialsFor(role, 'us-east-1')), 'function');
    const profile = authFor({ OYA_ECS_AUTH: 'sso', OYA_ECS_SSO_PROFILE: 'ops' });
    assert.equal(typeof (await credentialsFor(profile, 'us-east-1')), 'function');
  });

  it('exchanges an SSO access token for the role’s credentials', async () => {
    const send = mock.method(SSOClient.prototype, 'send', async () => ({
      roleCredentials: { accessKeyId: 'A', secretAccessKey: 'S', sessionToken: 'T', expiration: 1_900_000_000_000 },
    }));
    const provider = await credentialsFor(authFor(SSO), 'us-east-1');
    const credentials = await provider();
    assert.deepEqual([credentials.accessKeyId, credentials.sessionToken], ['A', 'T']);
    assert.ok(credentials.expiration instanceof Date, 'the SDK refreshes them before they expire');
    assert.deepEqual((send.mock.calls[0].arguments[0] as any).input, {
      accessToken: 'token',
      accountId: '123456789012',
      roleName: 'Browsers',
    });
  });

  it('says an expired SSO token needs replacing, and passes on any other failure', async () => {
    const expired = Object.assign(new Error('Session token not found or invalid'), { name: 'UnauthorizedException' });
    mock.method(SSOClient.prototype, 'send', async () => Promise.reject(expired));
    const provider = await credentialsFor(authFor(SSO), 'us-east-1');
    await assert.rejects(provider(), { status: Status.UNPROCESSABLE, code: 'sso_token_expired' });
    mock.restoreAll();
    mock.method(SSOClient.prototype, 'send', async () => Promise.reject(new Error('network down')));
    await assert.rejects((await credentialsFor(authFor(SSO), 'us-east-1'))(), /network down/);
  });
});
