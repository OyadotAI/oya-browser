/**
 * Unit tests for the ECS sandbox runtime with the SDK client's send stubbed:
 * the Fargate task it runs, what ECS failing to start one looks like, and
 * finding, stopping and listing tasks by startedBy and tags.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../../src/platform/http-status.ts';
import * as ecs from '../../../../../src/drivers/sandbox/workers/ecs.ts';

/** A complete configuration, on the key's own AWS account. */
const ENV = {
  OYA_ECS_CLUSTER: 'browsers',
  OYA_ECS_TASK_DEFINITION: 'arn:aws:ecs:us-east-1:1:task-definition/oya-browser:3',
  OYA_ECS_SUBNETS: 'subnet-a, subnet-b',
  OYA_ECS_SECURITY_GROUPS: 'sg-1',
  OYA_ECS_ASSIGN_PUBLIC_IP: 'true',
  AWS_REGION: 'us-east-1',
  OYA_ECS_AUTH: 'iam',
  AWS_ACCESS_KEY_ID: 'AKIA',
  AWS_SECRET_ACCESS_KEY: 'shh',
};
const CONFIG = ecs.settings(ENV)!;
/** What the facade hands every runtime. */
const SPEC = {
  name: 'oya-browser-b-1',
  labels: { 'oya-browser': 'true', 'oya-owner': 'owner-tag', 'oya-name': "Mo's #1 browser" },
  env: { OYA_API_KEY: 'secret-key', OYA_MAX_LIFETIME_MINUTES: '70' },
  ttlMinutes: 60,
  lifetimeMinutes: 70,
};

/** A task as DescribeTasks returns it. */
const task = (arn, tags) => ({
  taskArn: arn,
  lastStatus: 'RUNNING',
  tags: Object.entries(tags).map(([key, value]) => ({ key, value })),
});

/** Stubs send with a handler per command name, recording each call. */
async function stubSend(handlers) {
  const client = await ecs.client(CONFIG);
  return mock.method(client, 'send', async (command) => {
    const name = command.constructor.name.replace(/Command$/, '');
    return handlers[name](command.input);
  });
}

/** The inputs send was called with for one command. */
const inputs = (send, name) =>
  send.mock.calls
    .map((call) => call.arguments[0])
    .filter((c) => c.constructor.name === `${name}Command`)
    .map((c) => c.input);

afterEach(() => mock.restoreAll());

describe('ecs settings', () => {
  it('reads the network, the default container and the authentication', () => {
    assert.deepEqual(CONFIG.network.awsvpcConfiguration, {
      subnets: ['subnet-a', 'subnet-b'],
      securityGroups: ['sg-1'],
      assignPublicIp: 'ENABLED',
    });
    assert.equal(CONFIG.container, 'browser');
    assert.deepEqual(CONFIG.auth, { type: 'iam', accessKeyId: 'AKIA', secretAccessKey: 'shh' });
  });

  it('names what is missing, the authentication’s needs included', () => {
    assert.equal(ecs.settings({ ...ENV, OYA_ECS_SUBNETS: ' , ' }), null);
    assert.deepEqual(ecs.missing({}), ['OYA_ECS_CLUSTER', 'OYA_ECS_TASK_DEFINITION', 'OYA_ECS_SUBNETS', 'AWS_REGION']);
    assert.deepEqual(ecs.missing({ ...ENV, OYA_ECS_AUTH: 'role' }), ['OYA_ECS_ROLE_ARN']);
    assert.equal(ecs.settings({ ...ENV, OYA_ECS_AUTH: 'kerberos' }), null);
  });
});

describe('ecs create', () => {
  it('runs one Fargate task started by the sandbox name, labels as tags, env as an override', async () => {
    const send = await stubSend({ RunTask: () => ({ tasks: [{ taskArn: 'arn:task/1' }] }) });
    assert.deepEqual(await ecs.create(CONFIG, SPEC), { id: 'arn:task/1' });
    const [input] = inputs(send, 'RunTask');
    assert.deepEqual([input.launchType, input.startedBy, input.cluster], ['FARGATE', SPEC.name, 'browsers']);
    assert.deepEqual(input.overrides.containerOverrides[0].name, 'browser');
    assert.ok(input.overrides.containerOverrides[0].environment.some((e) => e.name === 'OYA_API_KEY'));
    assert.deepEqual(
      input.tags.find((t) => t.key === 'oya-name'),
      { key: 'oya-name', value: 'Mo-s -1 browser' },
      'tag values keep only what ECS accepts',
    );
  });

  it('says why when ECS starts nothing', async () => {
    await stubSend({ RunTask: () => ({ tasks: [], failures: [{ reason: 'RESOURCE:ENI', detail: 'no capacity' }] }) });
    await assert.rejects(ecs.create(CONFIG, SPEC), {
      status: Status.BAD_GATEWAY,
      message: /RESOURCE:ENI \(no capacity\)/,
    });
  });
});

describe('ecs find, destroy and list', () => {
  it('finds the running task by startedBy and stops it', async () => {
    const send = await stubSend({
      ListTasks: () => ({ taskArns: ['arn:task/1'] }),
      DescribeTasks: () => ({ tasks: [task('arn:task/1', { 'oya-browser-id': 'b-1' })] }),
      StopTask: () => ({}),
    });
    const found = await ecs.find(CONFIG, SPEC.name);
    assert.deepEqual([found!.labels, found!.state], [{ 'oya-browser-id': 'b-1' }, 'running']);
    assert.equal(inputs(send, 'ListTasks')[0].startedBy, SPEC.name);
    await found!.destroy();
    assert.equal(inputs(send, 'StopTask')[0].task, 'arn:task/1');
  });

  it('answers null when no task was started as that sandbox', async () => {
    await stubSend({ ListTasks: () => ({ taskArns: [] }) });
    assert.equal(await ecs.find(CONFIG, SPEC.name), null);
  });

  it('lists every page of the task family, describing at most 100 tasks at a time', async () => {
    const arns = Array.from({ length: 150 }, (_, i) => `arn:task/${i}`);
    const send = await stubSend({
      ListTasks: (input) =>
        input.nextToken ? { taskArns: arns.slice(100) } : { taskArns: arns.slice(0, 100), nextToken: 'p2' },
      DescribeTasks: (input) => ({ tasks: input.tasks.map((arn) => task(arn, { 'oya-owner': 'owner-tag' })) }),
    });
    assert.equal((await ecs.list(CONFIG, 'owner-tag')).length, 150);
    assert.equal(inputs(send, 'ListTasks')[0].family, 'oya-browser');
    assert.deepEqual(
      inputs(send, 'DescribeTasks').map((input) => input.tasks.length),
      [100, 50],
    );
  });
});
