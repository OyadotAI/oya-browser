/**
 * What a person's plan lets them start: cloud browsers (time and how many at
 * once), agent steps, and the hosted model. Checked per person, not per key:
 * a person can make any number of keys, and every one of them spends the same
 * allowance. Unless the license says this is Oya's own hosted deployment,
 * cloud browsers are also held to the self-hosted license's cap.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { hosted } from './config.ts';
import { MODEL_PRICES, PLANS, SALES_EMAIL, type Plan } from './constants.ts';
import { includedFor, type CreditReader } from './adjustments.ts';
import { priced } from './cost.ts';
import { Reservations, SERVER } from './reservations.ts';
import { standingOf, type Standing } from './standing.ts';
import type { Subscription } from './repository.ts';
import type { Admission } from '../../platform/license/index.ts';

/** What admission needs from the rest of the server. */
export type EntitlementDeps = CreditReader & {
  /** A person's subscription row. */
  find(userId: string): Promise<Subscription | null>;
  /** A person's billed usage since an ISO time. */
  usageSince(userId: string, since: string): Promise<Record<string, number>>;
  /** The person who owns a key; null for keys nobody owns (env keys, the fleet token). */
  ownerOf(key: string): Promise<string | null>;
  /** The digests of every key a person owns. */
  keyDigestsOf(userId: string): Promise<Set<string>>;
  /** A key's digest. */
  keyDigest(key: string): string;
  /** The key of every cloud browser this replica runs. */
  cloudKeys(): string[];
  /** Books a key's billed usage to its owner from now on. */
  billTo(key: string, userId: string): void;
  /** Whether a key brings its own model, so the hosted model's rules do not apply. */
  ownsModel(key: string): boolean;
  /** The model a key's runs use. */
  modelOf(key: string): string;
  /** The self-hosted license: whether this many cloud browsers at once is covered. */
  licenseAdmit(count: number): Admission;
  /** Whether the license says this is Oya's own hosted deployment, which has no self-hosted cap. */
  hostedDeployment(): boolean;
  /** Where a person upgrades. */
  upgradeUrl(): string;
  /** The time, in ms. */
  now(): number;
};

/** A plan's allowance by name, as `checkAllowance` reads it. */
type Allowance = keyof Pick<Plan, 'cloudSeconds' | 'steps' | 'llmMicroUsd'>;

/** Admission against a person's plan, and the self-hosted license. */
export class Entitlements {
  /** What admission reads. */
  declare deps: EntitlementDeps;
  /** Digests of keys whose owner is on a paid plan: only those get the operator's residential proxy. */
  declare paidKeys: Set<string>;
  /** Starts admitted but not connected yet. */
  declare reservations: Reservations;

  /** Admission reads everything through `deps`. */
  constructor(deps: EntitlementDeps) {
    this.deps = deps;
    this.paidKeys = new Set();
    this.reservations = new Reservations(deps.now);
  }

  /** Admits `requested` more cloud browsers for `key`, or throws the 402 that says why not. */
  async admitCloud(key: string, requested = 1) {
    if (!this.deps.hostedDeployment()) this.admitSelfHosted(requested);
    const standing = hosted() ? await this.standingFor(key) : null;
    if (standing) await this.checkConcurrency(standing, requested);
    if (standing) await this.checkAllowance(standing, 'cloud_seconds', 'cloudSeconds', 'cloud browser time');
    this.reservations.hold(SERVER, requested);
    if (standing) this.reservations.hold(standing.userId, requested);
  }

  /** Admits an agent step: the plan's steps, and on the hosted model its allowance and a model it prices. */
  async admitAgent(key: string) {
    if (!hosted()) return;
    const standing = await this.standingFor(key);
    if (!standing) return;
    await this.checkAllowance(standing, 'agent_steps', 'steps', 'agent steps');
    if (this.deps.ownsModel(key)) return;
    if (!priced(this.deps.modelOf(key))) throw unpricedModel(this.deps.modelOf(key));
    await this.checkAllowance(standing, 'hosted_llm_microusd', 'llmMicroUsd', 'hosted model use');
  }

  /** Whether a cloud browser on `key` may use the operator's residential proxy: only on a paid plan. */
  proxyAllowed(key: string) {
    return !hosted() || this.paidKeys.has(this.deps.keyDigest(key));
  }

  /** A cloud browser connected: its place is no longer held, and its owner's plan is learned before it is welcomed. */
  async attribute(key: string) {
    this.reservations.release(SERVER);
    if (!hosted()) return;
    const userId = await this.deps.ownerOf(key).catch(() => null);
    if (userId) this.reservations.release(userId);
    await this.standingFor(key).catch(() => null);
  }

  /** The self-hosted cap: every cloud browser running or on its way, plus these. */
  admitSelfHosted(requested: number) {
    const running = this.deps.cloudKeys().length + this.reservations.count(SERVER);
    const { ok, message } = this.deps.licenseAdmit(running + requested);
    if (!ok) throw new HttpError(Status.PAYMENT_REQUIRED, message, { code: 'license_required', contact: SALES_EMAIL });
  }

  /** The key's owner's standing, or null for a key nobody owns; a late payment is refused here. */
  async standingFor(key: string): Promise<Standing | null> {
    const userId = await this.deps.ownerOf(key);
    if (!userId) return null;
    this.deps.billTo(key, userId);
    const standing = standingOf(userId, await this.deps.find(userId), this.deps.now());
    this.noteProxy(key, standing);
    if (standing.status === 'past_due') throw this.refuse('payment_required', 'Your last payment failed.');
    return standing;
  }

  /** Remembers whether the key's owner pays, for the synchronous proxy check. */
  noteProxy(key: string, standing: Standing) {
    const digest = this.deps.keyDigest(key);
    if (standing.plan !== 'free' && standing.status !== 'past_due') this.paidKeys.add(digest);
    else this.paidKeys.delete(digest);
  }

  /** Refuses a start that would run more cloud browsers at once than the plan allows, across all the person's keys. */
  async checkConcurrency(standing: Standing, requested: number) {
    const mine = await this.deps.keyDigestsOf(standing.userId);
    const running = this.deps.cloudKeys().filter((k) => mine.has(this.deps.keyDigest(k))).length;
    const cap = PLANS[standing.plan].concurrent;
    if (running + this.reservations.count(standing.userId) + requested > cap)
      throw this.refuse('plan_concurrency', `The ${standing.plan} plan runs ${cap} cloud browsers at once.`);
  }

  /** Refuses once a plan without overage has used this period's allowance of `field`. */
  async checkAllowance(standing: Standing, field: string, allowance: Allowance, what: string) {
    const credits = await this.deps.creditsFor?.(standing.userId, standing.since);
    const plan = includedFor(standing, credits);
    if (plan.overage) return;
    const used = (await this.deps.usageSince(standing.userId, standing.since))[field] || 0;
    if (used >= plan[allowance])
      throw this.refuse('plan_limit', `This month's ${what} on the ${standing.plan} plan is used up.`);
  }

  /** The 402 a plan refusal answers with: what ran out, where to upgrade, and who to write to for more. */
  refuse(code: string, why: string) {
    const upgrade_url = this.deps.upgradeUrl();
    const message = `${why} Upgrade at ${upgrade_url}, or write to ${SALES_EMAIL} for more.`;
    return new HttpError(Status.PAYMENT_REQUIRED, message, { code, upgrade_url, contact: SALES_EMAIL });
  }
}

/** The 400 for a hosted model this server does not run on its own key. */
const unpricedModel = (model: string) =>
  new HttpError(
    Status.BAD_REQUEST,
    `The hosted model runs ${Object.keys(MODEL_PRICES).join(', ')}, not "${model}". Set your own model key to use another.`,
    { code: 'model_not_hosted' },
  );
