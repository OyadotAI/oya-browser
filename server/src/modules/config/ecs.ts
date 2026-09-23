/**
 * The `ecs` setting: one nested object with where a key's ECS browsers run and
 * how Oya authenticates to the key's AWS account, as the SDK sends it:
 *
 *   { cluster, taskDefinition, subnets: [..], securityGroups?: [..],
 *     assignPublicIp?, container?, region,
 *     auth: { type: 'iam', accessKeyId, secretAccessKey, sessionToken? }
 *         | { type: 'role', roleArn }
 *         | { type: 'sso', accessToken, accountId, roleName, ssoRegion? } }
 *
 * Checked strictly here, stored sealed as one secret, read back with its
 * credentials masked, and handed to the ECS worker as the OYA_ECS_* / AWS_*
 * names the deployment's own environment uses, so the worker reads both alike.
 * A role's ExternalId and an SSO profile are never accepted: the first is
 * Oya's per key, the second would be this host's own SSO session.
 */
import { invalid } from '../../platform/errors.ts';
import { CONFIG_VALUE_MAX_CHARS } from './constants.ts';

/** Checks one value and returns it normalised, or throws naming its path. */
type Rule = (value: unknown, path: string) => unknown;
/** How one key of an object is checked. */
type Key = {
  /** The check its value must pass. */
  rule: Rule;
  /** Whether it may be left out. */
  optional?: boolean;
};
/** Each key an object may hold. */
type Shape = Record<string, Key>;

/** A single-line string, matching `pattern` when one is given. */
const text =
  (pattern?: RegExp, needs = 'a non-empty string'): Rule =>
  (value, path) => {
    const ok = typeof value === 'string' && !!value.trim() && value.length <= CONFIG_VALUE_MAX_CHARS;
    if (!ok || /[\r\n]/.test(value) || (pattern && !pattern.test(value))) throw invalid(path, needs, value);
    return value.trim();
  };

/** A non-empty list of strings matching `pattern`. */
const list =
  (pattern: RegExp, needs: string): Rule =>
  (value, path) => {
    if (!Array.isArray(value) || !value.length) throw invalid(path, `a non-empty list of ${needs}`, value);
    return value.map((item, i) => text(pattern, needs)(item, `${path}[${i}]`));
  };

/** true or false. */
const flag: Rule = (value, path) => {
  if (typeof value !== 'boolean') throw invalid(path, 'true or false', value);
  return value;
};

/** An AWS region such as us-east-1. */
const region = text(/^[a-z]{2}(-[a-z]+)+-\d+$/, 'an AWS region such as us-east-1');

/** What each kind of authentication holds. */
const AUTH: Record<string, Shape> = {
  iam: {
    accessKeyId: { rule: text(/^[A-Z0-9]{16,128}$/, 'an AWS access key id') },
    secretAccessKey: { rule: text() },
    sessionToken: { rule: text(), optional: true },
  },
  role: { roleArn: { rule: text(/^arn:aws[\w-]*:iam::\d{12}:role\/[\w+=,.@/-]+$/, 'an IAM role ARN') } },
  sso: {
    accessToken: { rule: text() },
    accountId: { rule: text(/^\d{12}$/, 'a 12-digit AWS account id') },
    roleName: { rule: text(/^[\w+=,.@-]+$/, 'an IAM Identity Center role name') },
    ssoRegion: { rule: region, optional: true },
  },
};

/** The credentials in an auth object, sealed at rest and masked on read. */
const SECRET_KEYS = ['secretAccessKey', 'sessionToken', 'accessToken'];

/** Why an ExternalId is refused. */
const EXTERNAL_ID =
  'set by Oya, not by you: read it from config.get().ecs.externalId and put it in the role’s trust policy';

/** `auth`: one of the kinds, holding exactly what that kind needs. */
const auth: Rule = (value: any, path) => {
  if (value?.externalId !== undefined) throw invalid(`${path}.externalId`, EXTERNAL_ID, value.externalId);
  const type = value?.type;
  if (typeof type !== 'string' || !Object.hasOwn(AUTH, type))
    throw invalid(`${path}.type`, `one of: ${Object.keys(AUTH).join(', ')}`, type);
  const { type: _type, ...rest } = value;
  return { type, ...shaped(AUTH[type], rest, path) };
};

/** The whole setting. */
const ECS: Shape = {
  cluster: { rule: text() },
  taskDefinition: { rule: text() },
  subnets: { rule: list(/^subnet-[0-9a-f]+$/, 'subnet ids') },
  securityGroups: { rule: list(/^sg-[0-9a-f]+$/, 'security group ids'), optional: true },
  assignPublicIp: { rule: flag, optional: true },
  container: { rule: text(/^[\w-]+$/, 'a container name'), optional: true },
  region: { rule: region },
  auth: { rule: auth },
};

/** An object holding only `shape`'s keys, each passing its rule. */
function shaped(shape: Shape, value: any, path: string) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid(path, 'an object', value);
  const unknown = Object.keys(value).find((key) => !Object.hasOwn(shape, key));
  if (unknown) throw invalid(`${path}.${unknown}`, `one of: ${Object.keys(shape).join(', ')}`, value[unknown]);
  const present = Object.entries(shape).filter(([key, { optional }]) => value[key] !== undefined || !optional);
  return Object.fromEntries(present.map(([key, { rule }]) => [key, rule(value[key], `${path}.${key}`)]));
}

/**
 * The setting checked and normalised. What `config.get()` shows can be sent
 * straight back: a credential in its masked form keeps the stored one, and the
 * read-only externalId beside the setting is dropped.
 */
export function validateEcs(value, previous = null) {
  return shaped(ECS, withStoredSecrets(withoutShown(value), previous), 'ecs');
}

/** The value without the read-only externalId that config.get() shows beside the setting. */
function withoutShown(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return value;
  const { externalId: _shown, ...sent } = value;
  return sent;
}

/** Masked credentials replaced by the stored ones, when the auth kind has not changed. */
function withStoredSecrets(value, previous) {
  const sent = value?.auth;
  if (!sent || typeof sent !== 'object') return value;
  const kept = previous?.auth?.type === sent.type ? previous.auth : {};
  const masked = SECRET_KEYS.filter((key) => typeof sent[key] === 'string' && sent[key].startsWith('•'));
  return { ...value, auth: { ...sent, ...Object.fromEntries(masked.map((key) => [key, kept[key]])) } };
}

/** The setting with its credentials masked by `mask`, for reading back. */
export function viewEcs(ecs, mask: (v: string) => string) {
  const masked = SECRET_KEYS.filter((key) => ecs.auth?.[key]).map((key) => [key, mask(ecs.auth[key])]);
  return { ...ecs, auth: { ...ecs.auth, ...Object.fromEntries(masked) } };
}

/** How each kind of authentication is spelled in the environment the ECS worker reads. */
const AUTH_ENV = {
  iam: (a) => ({
    AWS_ACCESS_KEY_ID: a.accessKeyId,
    AWS_SECRET_ACCESS_KEY: a.secretAccessKey,
    AWS_SESSION_TOKEN: a.sessionToken,
  }),
  role: (a) => ({ OYA_ECS_ROLE_ARN: a.roleArn }),
  sso: (a) => ({
    OYA_ECS_SSO_ACCESS_TOKEN: a.accessToken,
    OYA_ECS_SSO_ACCOUNT_ID: a.accountId,
    OYA_ECS_SSO_ROLE_NAME: a.roleName,
    OYA_ECS_SSO_REGION: a.ssoRegion,
  }),
};

/** The setting as the environment names the ECS worker reads; unset values are left out. */
export function ecsEnv(ecs): Record<string, string> {
  const env = {
    ...placement(ecs),
    AWS_REGION: ecs.region,
    OYA_ECS_AUTH: ecs.auth.type,
    ...AUTH_ENV[ecs.auth.type](ecs.auth),
  };
  return Object.fromEntries(Object.entries(env).filter(([, value]) => value)) as Record<string, string>;
}

/** Where the task runs, as environment variables. */
const placement = (ecs) => ({
  OYA_ECS_CLUSTER: ecs.cluster,
  OYA_ECS_TASK_DEFINITION: ecs.taskDefinition,
  OYA_ECS_SUBNETS: ecs.subnets.join(','),
  OYA_ECS_SECURITY_GROUPS: (ecs.securityGroups || []).join(','),
  OYA_ECS_ASSIGN_PUBLIC_IP: ecs.assignPublicIp ? 'true' : '',
  OYA_ECS_CONTAINER: ecs.container,
});
