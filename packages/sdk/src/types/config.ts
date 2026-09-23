/**
 * Types for a key's settings: the LLM behind `ask()`, the browser provider and
 * the CAPTCHA solver, as `config.set()` accepts them and `config.get()` returns them.
 */
import type { Provider } from './browsers.js';

/** Which model drives `ask()` and the chat API. */
export type LlmProvider =
  | 'openai'
  | 'anthropic'
  /** Gemini via AI Studio. */
  | 'gemini'
  /** Gemini Enterprise, ex-Vertex AI. Express mode by default; set `openai_base_url` to a
   *  project-scoped `.../endpoints/openapi` endpoint to use an enterprise project. */
  | 'vertex';

/**
 * Which runtime this key's `'oya-cloud'` browsers run on. `'docker'` and `'k8s'`
 * run on the deployment's own daemon or cluster, exactly as its operator set them up;
 * `'daytona'` and `'ecs'` run on your own account once you set that runtime's
 * credentials below, and on the deployment's otherwise.
 */
export type SandboxRuntime = 'daytona' | 'docker' | 'k8s' | 'ecs';

/**
 * How Oya signs in to your AWS account to run `'ecs'` browsers.
 *
 * - `iam`: an access key (optionally temporary, with its session token).
 * - `role`: Oya assumes `roleArn` from its own AWS identity. Put Oya's AWS
 *   account and `config.get().ecs.externalId` in the role's trust policy; the
 *   ExternalId is Oya's, one per API key, and cannot be set.
 * - `sso`: an IAM Identity Center access token (from `aws sso login`'s cache or
 *   the SSO OIDC device flow) for `accountId` and `roleName`. It expires; starts
 *   then fail with `sso_token_expired` until you set a fresh one.
 */
export type EcsAuth =
  | {
      /** An IAM access key. */
      type: 'iam';
      /** The access key id. */
      accessKeyId: string;
      /** Its secret. Read back masked. */
      secretAccessKey: string;
      /** For temporary credentials. Read back masked. */
      sessionToken?: string;
    }
  | {
      /** A role Oya assumes. */
      type: 'role';
      /** e.g. arn:aws:iam::123456789012:role/oya-browsers */
      roleArn: string;
    }
  | {
      /** IAM Identity Center. */
      type: 'sso';
      /** The SSO access token. Read back masked. */
      accessToken: string;
      /** The 12-digit account to sign in to. */
      accountId: string;
      /** The permission set's role name in that account. */
      roleName: string;
      /** The SSO portal's region, when it differs from `region`. */
      ssoRegion?: string;
    };

/** Where `'ecs'` browsers run: one Fargate task each, of a task definition that runs the Oya browser image. */
export interface EcsConfig {
  /** The ECS cluster. */
  cluster: string;
  /** The task definition: family, family:revision or ARN. */
  taskDefinition: string;
  /** Subnet ids for the tasks, e.g. ['subnet-0abc']. */
  subnets: string[];
  /** Security group ids for the tasks. */
  securityGroups?: string[];
  /** Give each task a public IP; needed in a public subnet without a NAT gateway. */
  assignPublicIp?: boolean;
  /** The container in the task definition that runs the browser; defaults to 'browser'. */
  container?: string;
  /** The AWS region, e.g. 'us-east-1'. */
  region: string;
  /** How Oya signs in to your account. */
  auth: EcsAuth;
}

/** Empty disables solving. */
export type CaptchaSolver = 'capsolver' | '2captcha' | '';

/**
 * What a key can configure. Mirrors the server's field allowlist exactly: a field not
 * listed here is ignored rather than stored, so the type is the whole surface.
 * `null` clears a field and falls back to the deployment default.
 */
export interface ConfigUpdate {
  /** Which LLM vendor `ask()` and the chat API call. */
  llm_provider?: LlmProvider | null;
  /** The credential for whichever `llm_provider` is set, the field name is shared. */
  openai_api_key?: string | null;
  /** Only honoured alongside this key's own `openai_api_key`. */
  openai_base_url?: string | null;
  /** Overrides the provider's default model. */
  chat_model?: string | null;
  /** Where this key's browsers run unless `start()` names a provider. */
  browser_provider?: Provider | null;
  /** Anchor's API key, for the 'anchor' provider. */
  anchor_api_key?: string | null;
  /** Browserbase's API key, for the 'browserbase' provider. */
  browserbase_api_key?: string | null;
  /** The Browserbase project browsers are created in. */
  browserbase_project_id?: string | null;
  /** Steel's API key, for the 'steel' provider. */
  steel_api_key?: string | null;
  /** Browser Use Cloud's API key, for the 'browseruse' provider. */
  browseruse_api_key?: string | null;
  /** The DevTools WebSocket URL of your own Chrome, for the 'cdp' provider. */
  cdp_ws_url?: string | null;
  /** Which runtime `'oya-cloud'` browsers run on; unset uses the deployment's. */
  sandbox_runtime?: SandboxRuntime | null;
  /** Your Daytona API key, so `'daytona'` runs on your own Daytona account. */
  daytona_api_key?: string | null;
  /** The Daytona snapshot of the Oya browser image, in your account. */
  daytona_snapshot?: string | null;
  /** The Daytona region, e.g. 'us' or 'eu'. */
  daytona_target?: string | null;
  /** Where this key's `'ecs'` browsers run and how Oya signs in to your AWS account. */
  ecs?: EcsConfig | null;
  /** Which CAPTCHA solving service to call. */
  captcha_solver?: CaptchaSolver | null;
  /** The solving service's API key. */
  captcha_api_key?: string | null;
  /** Set once onboarding has been completed, so the console stops offering it. */
  onboarded?: string | null;
}

/** What `config.get()` returns. Secrets read back masked, never in full. */
export interface Config extends Omit<
  ConfigUpdate,
  'llm_provider' | 'browser_provider' | 'captcha_solver' | 'sandbox_runtime' | 'ecs'
> {
  /** The configured sandbox runtime, or empty for the deployment's. */
  sandbox_runtime?: SandboxRuntime | '';
  /** The ECS setting with its credentials masked, and the ExternalId to trust for `auth.type: 'role'`. */
  ecs: Partial<EcsConfig> & {
    /** Put this in your role's trust policy as sts:ExternalId. Shown even before ECS is set. */
    externalId: string;
  };
  /** The configured LLM vendor, or empty for the deployment default. */
  llm_provider?: LlmProvider | '';
  /** The configured browser provider, or empty for the deployment default. */
  browser_provider?: Provider | '';
  /** The configured CAPTCHA solver; empty when solving is off. */
  captcha_solver?: CaptchaSolver;
  /** What this key would actually use right now, deployment defaults included. */
  effective: {
    /** The LLM endpoint in use. */
    baseUrl: string;
    /** The model in use. */
    model: string;
    /** Whether any LLM credential is available. */
    hasLlmKey: boolean;
  };
  /** True when the LLM key in play belongs to the deployment, not this key. */
  inherited: boolean;
  /** Whether this key has its own LLM credential stored. */
  has_openai_key: boolean;
  /** Every browser provider, what it needs configured, and whether it is. */
  providers: Array<{
    /** The provider. */
    id: Provider;
    /** Its display name. */
    label: string;
    /** The config fields it requires. */
    needs: string[];
    /** Whether those fields are set. */
    configured: boolean;
  }>;
}
