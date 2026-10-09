/** Fail-closed installation of the implemented native policy subset, without any CDP fallback. */
import { POLICY_METHODS } from './constants.ts';
import type { NativePolicy, PolicyBinding, PolicyEngine, PolicySession } from './types.ts';
import { validatePolicy } from './validate.ts';
import { readState, verifyInstalled } from './state.ts';

/** Detect an incomplete engine before making even the first irreversible setter call. */
function engineFor(session: PolicySession): PolicyEngine {
  if (POLICY_METHODS.some((name) => typeof session[name] !== 'function'))
    throw new Error('Unsupported native session policy capability; a patched Oya engine is required');
  const engine = session as PolicyEngine;
  if (readState(engine).rendererStarted) throw new Error('Native policy must be configured before any renderer starts');
  return engine;
}
/** A session can only be reused for the identical fully installed policy. */
function reuse(binding: PolicyBinding, policy: NativePolicy): NativePolicy {
  if (binding.state !== 'installed') throw new Error('Native policy session is unavailable; retire this session');
  if (JSON.stringify(binding.policy) !== JSON.stringify(policy)) throw new Error('Native session policy cannot change');
  return binding.policy;
}
/** Commit only validated values through the browser-owned native API, retaining the receiver. */
function installValues(engine: PolicyEngine, policy: NativePolicy): void {
  engine._setOyaTimeZone(policy.timeZone);
  engine._setOyaHardwareConcurrency(policy.hardwareConcurrency);
  engine._setOyaLocale(policy.locale);
}
/** Publish completion only after native readback agrees with every requested value. */
function installed(binding: PolicyBinding): NativePolicy {
  binding.state = 'installed';
  return binding.policy;
}
/** Record failure permanently because native setters cannot safely be rolled back. */
function commitNativePolicy(engine: PolicyEngine, binding: PolicyBinding): NativePolicy {
  try {
    installValues(engine, binding.policy);
    verifyInstalled(engine, binding.policy);
    return installed(binding);
  } catch (cause) {
    binding.state = 'failed';
    throw new Error('Native policy installation failed; retire this session', { cause });
  }
}
/** Owned by one application lifecycle; completion covers only this explicit policy subset. */
export class NativeSessionPolicies {
  /** Weak keys preserve exact session identity without keeping disposed sessions alive. */
  private readonly bindings = new WeakMap<PolicySession, PolicyBinding>();

  /** Validate all input/capabilities first, then install once or require identical reuse. */
  configure(session: PolicySession, input: unknown): NativePolicy {
    const policy = validatePolicy(input);
    const existing = this.bindings.get(session);
    if (existing) return reuse(existing, policy);
    const engine = engineFor(session);
    const binding: PolicyBinding = { policy, state: 'installing' };
    this.bindings.set(session, binding);
    return commitNativePolicy(engine, binding);
  }

  /** Refuse exposure until this owner has installed the complete supported subset. */
  assertConfigured(session: PolicySession): NativePolicy {
    const binding = this.bindings.get(session);
    if (!binding) throw new Error('Native session policy has not been configured');
    return reuse(binding, binding.policy);
  }
}
