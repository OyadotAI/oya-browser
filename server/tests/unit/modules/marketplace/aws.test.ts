/** AWS exchange tests pin safe errors, hard caller deadlines and bounded outstanding SDK work. */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { AwsBuyerResolver } from '../../../../src/modules/marketplace/aws.ts';
import { AWS_CONCURRENCY, AWS_TIMEOUT_MS } from '../../../../src/modules/marketplace/constants.ts';

afterEach(() => mock.timers.reset());

describe('AWS buyer resolution', () => {
  it('uses the opaque token only in the AWS command and reads the current identifiers', async () => {
    const send = mock.fn(async () => ({
      CustomerAWSAccountId: '123456789012',
      LicenseArn: 'arn:test',
      ProductCode: 'p',
    }));
    const factory = mock.fn(() => ({ send }));
    const resolver = new AwsBuyerResolver(factory);
    assert.deepEqual(await resolver.resolve('opaque-registration-token', 'us-east-1'), {
      accountId: '123456789012',
      licenseArn: 'arn:test',
      productCode: 'p',
    });
    assert.deepEqual(send.mock.calls[0].arguments[0].input, { RegistrationToken: 'opaque-registration-token' });
    assert.deepEqual(factory.mock.calls[0].arguments, ['us-east-1']);
  });

  it('redacts provider exceptions and releases capacity after a rejected SDK call', async () => {
    const error = new Error('secret AWS token and credentials');
    error.name = 'ExpiredTokenException';
    const resolver = new AwsBuyerResolver(() => ({
      send: async () => {
        throw error;
      },
    }));
    for (let i = 0; i <= AWS_CONCURRENCY; i++)
      await assert.rejects(resolver.resolve('token', 'us-east-1'), {
        code: 'invalid_setup',
        message: 'Return to AWS Marketplace',
      });
    error.name = 'InternalServiceErrorException';
    await assert.rejects(resolver.resolve('token', 'us-east-1'), {
      code: 'unavailable',
      message: 'Setup is temporarily unavailable',
    });
  });

  it('enforces the caller deadline even if credential work ignores abort, without admitting unlimited stranded work', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const send = mock.fn(() => new Promise(() => {}));
    const resolver = new AwsBuyerResolver(() => ({ send }));
    const work = Array.from({ length: AWS_CONCURRENCY }, () =>
      assert.rejects(resolver.resolve('token', 'us-east-1'), { code: 'unavailable' }),
    );
    await Promise.resolve();
    await assert.rejects(resolver.resolve('token', 'us-east-1'), { code: 'unavailable' });
    mock.timers.tick(AWS_TIMEOUT_MS);
    await Promise.all(work);
    await assert.rejects(resolver.resolve('token', 'us-east-1'), { code: 'unavailable' });
    assert.equal(send.mock.callCount(), AWS_CONCURRENCY);
  });
});
