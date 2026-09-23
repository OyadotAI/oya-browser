/**
 * Amazon ECS sandbox runtime: one Fargate task of a task definition that runs
 * the browser image, per browser. The deployment configures it with OYA_ECS_*
 * and the default AWS credential chain; a key that brings its own AWS keys runs
 * on its own account. The SDK is loaded lazily so the server runs without it
 * when this runtime is not in use.
 *
 * ECS tasks have no name, so a task is found by startedBy, which is set to the
 * sandbox name. Its labels are its tags. ECS has no idle stop; the image stops
 * itself at OYA_MAX_LIFETIME_MINUTES and the task ends with it.
 */
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import { unset } from '../names.ts';
import { authFor, authMissing, credentialsFor, ownAccount } from './ecs-auth.ts';
import { ECS_DESCRIBE_BATCH, SANDBOX_CLIENT_CACHE_MAX } from '../../constants.ts';

/** The runtime's name, as OYA_CLOUD_RUNTIME selects it. */
export const id = 'ecs';

export { ownAccount };

/** The container in the task definition that runs the browser, when OYA_ECS_CONTAINER is unset. */
const DEFAULT_CONTAINER = 'browser';

/** Longest value ECS accepts for a tag. */
const TAG_VALUE_MAX = 256;

/** Comma-separated ids as a list. */
const ids = (value) =>
  String(value || '')
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean);

/** The ECS settings from env, or null unless the cluster, task definition, subnets and region are set. */
export function settings(env) {
  if (missing(env).length) return null;
  const task = { cluster: env.OYA_ECS_CLUSTER, taskDefinition: env.OYA_ECS_TASK_DEFINITION };
  const container = env.OYA_ECS_CONTAINER || DEFAULT_CONTAINER;
  return { ...task, network: networkFor(env), container, region: env.AWS_REGION, auth: authFor(env) };
}

/** What is unset. */
export const missing = (env) =>
  unset([
    ['OYA_ECS_CLUSTER', env.OYA_ECS_CLUSTER],
    ['OYA_ECS_TASK_DEFINITION', env.OYA_ECS_TASK_DEFINITION],
    ['OYA_ECS_SUBNETS', ids(env.OYA_ECS_SUBNETS).join(',')],
    ['AWS_REGION', env.AWS_REGION],
  ]).concat(authMissing(env));

/** The awsvpc network a Fargate task needs. */
const networkFor = (env) => ({
  awsvpcConfiguration: {
    subnets: ids(env.OYA_ECS_SUBNETS),
    securityGroups: ids(env.OYA_ECS_SECURITY_GROUPS),
    assignPublicIp: env.OYA_ECS_ASSIGN_PUBLIC_IP === 'true' ? 'ENABLED' : 'DISABLED',
  },
});

/** Clients by region and authentication, so keys on their own accounts each get their own. */
const clients = new Map();

/**
 * The ECS client for these settings, created once per region and authentication,
 * so an assumed role or SSO session is reused and refreshed rather than re-made per call.
 */
export async function client(config) {
  const cacheKey = `${config.region}\n${JSON.stringify(config.auth)}`;
  if (!clients.has(cacheKey)) remember(cacheKey, loadSdk(config, cacheKey));
  return clients.get(cacheKey);
}

/** Caches a client, dropping the oldest past the cap. */
function remember(cacheKey, promise) {
  if (clients.size >= SANDBOX_CLIENT_CACHE_MAX) clients.delete(clients.keys().next().value);
  clients.set(cacheKey, promise);
}

/** Builds the client; a failure is forgotten so it can be retried. */
function loadSdk(config, cacheKey) {
  return newClient(config).catch((err) => {
    clients.delete(cacheKey);
    throw err instanceof HttpError ? err : unavailable(err);
  });
}

/** Imports the SDK and makes a client with these settings' credentials. */
async function newClient(config) {
  const { ECSClient } = await import('@aws-sdk/client-ecs');
  return new ECSClient({ region: config.region, credentials: await credentialsFor(config.auth, config.region) });
}

/** The 409 for an SDK that will not load. */
const unavailable = (err) =>
  new HttpError(
    Status.CONFLICT,
    `ECS runtime unavailable: ${err.message}. Run \`npm i @aws-sdk/client-ecs\` in server/.`,
  );

/** Sends one ECS command by name, e.g. send(config, 'RunTask', input). */
async function send(config, command: string, input) {
  const ecs = await client(config);
  const sdk = await import('@aws-sdk/client-ecs');
  return ecs.send(new sdk[`${command}Command`](input));
}

/** Runs the browser's task; resolves with its ARN once ECS has accepted it. */
export async function create(config, spec) {
  const out = await send(config, 'RunTask', runTask(config, spec));
  const task = out.tasks?.[0];
  if (!task) throw new HttpError(Status.BAD_GATEWAY, `ECS did not start the browser: ${failure(out)}`);
  return { id: task.taskArn };
}

/** Why RunTask started nothing, as ECS put it. */
const failure = (out) =>
  (out.failures || []).map((f) => `${f.reason}${f.detail ? ` (${f.detail})` : ''}`).join('; ') || 'no reason given';

/** The RunTask input: Fargate, startedBy the sandbox name, labels as tags, the environment as a container override. */
const runTask = (config, spec) => ({
  cluster: config.cluster,
  taskDefinition: config.taskDefinition,
  launchType: 'FARGATE',
  count: 1,
  startedBy: spec.name,
  networkConfiguration: config.network,
  tags: Object.entries(spec.labels).map(([key, value]) => ({ key, value: tagValue(value) })),
  overrides: { containerOverrides: [{ name: config.container, environment: toPairs(spec.env) }] },
});

/** ECS tag values allow letters, digits, spaces and _.:/=+-@ only, up to 256 characters. */
const tagValue = (value) =>
  String(value)
    .replace(/[^\p{L}\p{N} _.:/=+\-@]/gu, '-')
    .slice(0, TAG_VALUE_MAX);

/** An env object as ECS name/value pairs. */
const toPairs = (env) => Object.entries(env).map(([name, value]) => ({ name, value: String(value) }));

/** The running task started as this sandbox, or null when there is none. */
export async function find(config, name) {
  const listed = await send(config, 'ListTasks', { cluster: config.cluster, startedBy: name });
  const [task] = await describe(config, listed.taskArns || []);
  if (!task) return null;
  return { ...found(task), destroy: () => stop(config, task.taskArn) };
}

/** Stops the task. */
async function stop(config, taskArn) {
  await send(config, 'StopTask', { cluster: config.cluster, task: taskArn, reason: 'Oya browser stopped' });
}

/** Every running task of the browser's task definition; the caller keeps this owner's by their tags. */
export async function list(config, _owner) {
  return (await describe(config, await taskArns(config))).map(found);
}

/** Every running task ARN of the browser's task family, across all pages. */
async function taskArns(config) {
  const query = { cluster: config.cluster, family: familyOf(config.taskDefinition) };
  const arns = [];
  for (let nextToken, first = true; first || nextToken; first = false) {
    const page = await send(config, 'ListTasks', { ...query, nextToken });
    arns.push(...(page.taskArns || []));
    nextToken = page.nextToken;
  }
  return arns;
}

/** A task definition's family, from a family, family:revision or an ARN. */
const familyOf = (taskDefinition) => taskDefinition.split('/').pop().split(':')[0];

/** The tasks with their tags, a batch at a time as DescribeTasks allows. */
async function describe(config, arns) {
  const tasks = [];
  for (let i = 0; i < arns.length; i += ECS_DESCRIBE_BATCH) {
    const batch = arns.slice(i, i + ECS_DESCRIBE_BATCH);
    const out = await send(config, 'DescribeTasks', { cluster: config.cluster, tasks: batch, include: ['TAGS'] });
    tasks.push(...(out.tasks || []));
  }
  return tasks;
}

/** A task's labels (its tags) and state. */
const found = (task) => ({
  labels: Object.fromEntries((task.tags || []).map((tag) => [tag.key, tag.value])),
  state: String(task.lastStatus || 'unknown').toLowerCase(),
});
