/**
 * How the ECS worker authenticates to AWS, chosen by OYA_ECS_AUTH:
 *
 *   default  the AWS SDK's default chain: env keys, AWS_PROFILE (SSO included),
 *            an instance, task or IRSA role. The deployment's only.
 *   iam      an access key (AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY, optional
 *            AWS_SESSION_TOKEN).
 *   role     assume OYA_ECS_ROLE_ARN from the default chain's identity. On a
 *            key's own account the ExternalId is Oya's, one per key, never the
 *            key's choice (tenancy.ts), so no key can have this server assume
 *            a role another customer made for it.
 *   sso      IAM Identity Center: the deployment's OYA_ECS_SSO_PROFILE (after
 *            `aws sso login` on this host), or a key's own access token with
 *            its account and role, exchanged for role credentials as needed.
 *            A token expires; starts fail with sso_token_expired until a
 *            fresh one is set.
 */
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import { unset } from '../names.ts';

/** Every browser the role or SSO credentials start carries this session name in CloudTrail. */
const SESSION_NAME = 'oya-browser';

/** How one kind of AWS authentication reads its settings, names what it lacks, and makes credentials. */
type AuthKind = {
  /** Its settings from env. */
  read: (env) => Record<string, any>;
  /** The env names it still needs. */
  missing: (env) => string[];
  /** Whether a key's own settings are a complete account of its own (not the deployment's). */
  own: (env) => boolean;
  /** Credentials or a credential provider for the ECS client; undefined for the default chain. */
  credentials: (auth, region: string) => Promise<any>;
};

/** An access key from env. */
const accessKey = (env) => ({
  accessKeyId: env.AWS_ACCESS_KEY_ID,
  secretAccessKey: env.AWS_SECRET_ACCESS_KEY,
  ...(env.AWS_SESSION_TOKEN ? { sessionToken: env.AWS_SESSION_TOKEN } : {}),
});

/** The kinds of authentication, by the name OYA_ECS_AUTH selects them with. */
const KINDS: Record<string, AuthKind> = {
  default: { read: () => ({}), missing: () => [], own: () => false, credentials: async () => undefined },
  iam: {
    read: accessKey,
    missing: (env) =>
      unset([
        ['AWS_ACCESS_KEY_ID', env.AWS_ACCESS_KEY_ID],
        ['AWS_SECRET_ACCESS_KEY', env.AWS_SECRET_ACCESS_KEY],
      ]),
    own: (env) => !!(env.AWS_ACCESS_KEY_ID && env.AWS_SECRET_ACCESS_KEY),
    credentials: async ({ accessKeyId, secretAccessKey, sessionToken }) => ({
      accessKeyId,
      secretAccessKey,
      sessionToken,
    }),
  },
  role: {
    read: (env) => ({ roleArn: env.OYA_ECS_ROLE_ARN, externalId: env.OYA_ECS_EXTERNAL_ID || null }),
    missing: (env) => unset([['OYA_ECS_ROLE_ARN', env.OYA_ECS_ROLE_ARN]]),
    own: (env) => !!env.OYA_ECS_ROLE_ARN,
    credentials: assumedRole,
  },
  sso: {
    read: (env) => (env.OYA_ECS_SSO_ACCESS_TOKEN ? ssoToken(env) : { profile: env.OYA_ECS_SSO_PROFILE }),
    missing: ssoMissing,
    own: (env) => !!env.OYA_ECS_SSO_ACCESS_TOKEN,
    credentials: (auth) => (auth.accessToken ? ssoTokenCredentials(auth) : ssoProfile(auth)),
  },
};

/** The authentication types OYA_ECS_AUTH can name. */
export const AUTH_TYPES = Object.keys(KINDS);

/** The kind env selects, or null for a name that is not one. */
const kindOf = (env) => {
  const type = env.OYA_ECS_AUTH || 'default';
  return Object.hasOwn(KINDS, type) ? { type, kind: KINDS[type] } : null;
};

/** The authentication settings from env, or null when the type is unknown. */
export function authFor(env) {
  const chosen = kindOf(env);
  return chosen && { type: chosen.type, ...chosen.kind.read(env) };
}

/** The env names authentication still needs. */
export function authMissing(env) {
  const chosen = kindOf(env);
  return chosen ? chosen.kind.missing(env) : [`OYA_ECS_AUTH (one of ${AUTH_TYPES.join(', ')})`];
}

/** Whether env is a complete AWS account of a key's own, rather than the deployment's. */
export const ownAccount = (env) => !!kindOf(env)?.kind.own(env);

/** Credentials or a credential provider for these settings; undefined for the default chain. */
export const credentialsFor = (auth, region) => KINDS[auth.type].credentials(auth, region);

/** A key's SSO access token, the account and role it signs in to, and the SSO portal's region. */
const ssoToken = (env) => ({
  accessToken: env.OYA_ECS_SSO_ACCESS_TOKEN,
  accountId: env.OYA_ECS_SSO_ACCOUNT_ID,
  roleName: env.OYA_ECS_SSO_ROLE_NAME,
  ssoRegion: env.OYA_ECS_SSO_REGION || env.AWS_REGION,
});

/** A token needs its account and role; without a token, the deployment's profile. */
function ssoMissing(env) {
  if (!env.OYA_ECS_SSO_ACCESS_TOKEN) return unset([['OYA_ECS_SSO_PROFILE', env.OYA_ECS_SSO_PROFILE]]);
  return unset([
    ['OYA_ECS_SSO_ACCOUNT_ID', env.OYA_ECS_SSO_ACCOUNT_ID],
    ['OYA_ECS_SSO_ROLE_NAME', env.OYA_ECS_SSO_ROLE_NAME],
  ]);
}

/** The role, assumed from the default chain's identity, refreshed before it expires. */
async function assumedRole(auth, region) {
  const { fromTemporaryCredentials } = await import('@aws-sdk/credential-providers');
  const params = { RoleArn: auth.roleArn, RoleSessionName: SESSION_NAME };
  return fromTemporaryCredentials({
    params: auth.externalId ? { ...params, ExternalId: auth.externalId } : params,
    clientConfig: { region },
  });
}

/** The deployment's SSO profile, from the token cache `aws sso login` left on this host. */
async function ssoProfile(auth) {
  const { fromSSO } = await import('@aws-sdk/credential-providers');
  return fromSSO({ profile: auth.profile });
}

/** A provider that exchanges a key's SSO access token for its role's credentials, each time they expire. */
async function ssoTokenCredentials(auth) {
  const { SSOClient, GetRoleCredentialsCommand } = await import('@aws-sdk/client-sso');
  const client = new SSOClient({ region: auth.ssoRegion });
  const input = { accessToken: auth.accessToken, accountId: auth.accountId, roleName: auth.roleName };
  return async () => roleCredentials(await client.send(new GetRoleCredentialsCommand(input)).catch(ssoRefused));
}

/** What GetRoleCredentials answers. */
type RoleCredentialsOutput = {
  /** The role's temporary credentials. */
  roleCredentials?: any;
};

/** GetRoleCredentials' answer as SDK credentials. */
const roleCredentials = ({ roleCredentials: c }: RoleCredentialsOutput) => ({
  accessKeyId: c.accessKeyId,
  secretAccessKey: c.secretAccessKey,
  sessionToken: c.sessionToken,
  expiration: new Date(c.expiration),
});

/** An expired or revoked token says so, so the key knows to set a fresh one. */
function ssoRefused(err): never {
  if (err?.name !== 'UnauthorizedException') throw err;
  const message =
    'The ECS SSO access token has expired or was revoked; set a fresh one with config.set({ ecs: { auth } })';
  throw new HttpError(Status.UNPROCESSABLE, message, { code: 'sso_token_expired' });
}
