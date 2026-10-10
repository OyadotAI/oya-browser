/** Install immutable document and worker protection before a session creates any renderer. */
import { NativeSessionPolicies } from './install.ts';
import type { PolicySession } from './types.ts';
/** Exact original sources executed by the engine before website code. */
export interface PreScriptPolicy {
  /** Main-world document source, including initial empty documents and popups. */
  page: string;
  /** Dedicated, shared and service worker source. */
  worker: string;
}
/** Patched engine contract that cannot be satisfied by JavaScript-only preload registration. */
export interface ProtectedSession extends PolicySession {
  /** Install source before any document or worker renderer exists. */
  _setOyaPreScriptPolicy?: (sources: PreScriptPolicy) => void;
}
/** One immutable protection attempt per exact session, failed attempts permanently quarantined. */
interface ProtectionBinding {
  /** Normalized complete request used to detect identity changes. */
  key: string;
  /** Installation becomes visible only after authoritative native readback. */
  installed: boolean;
}
/** Engine contract revision for pre-script policy installation. */
const PRE_SCRIPT_VERSION = 1;
/** Verify engine support before any scalar policy mutation. */
function protectionState(session: ProtectedSession): Record<string, unknown> {
  const state = session._getOyaSessionPolicy?.() as Record<string, unknown> | undefined;
  if (!session._setOyaPreScriptPolicy || state?.preScriptPolicyVersion !== PRE_SCRIPT_VERSION)
    throw Error('Unsupported native pre-script protection; a complete patched Oya engine is required');
  return state;
}
/** Verify exact installed sources; native setters must not silently discard them. */
function verifyProtectionSources(session: ProtectedSession, sources: PreScriptPolicy): void {
  const state = protectionState(session);
  const actual = state.preScriptPolicy as PreScriptPolicy | undefined;
  if (state.rendererStarted || actual?.page !== sources.page || actual?.worker !== sources.worker)
    throw Error('Native pre-script protection readback mismatch');
}
/** Own complete protection across tab creation, private contexts and persona reconnects. */
export class NativeSessionProtection {
  /** Scalar policy ownership shares the same session lifecycle. */
  private readonly policies = new NativeSessionPolicies();
  /** Failed sessions cannot be reused or mistaken for protected sessions. */
  private readonly bindings = new WeakMap<ProtectedSession, ProtectionBinding>();
  /** Install once, or demand an identical immutable policy on reconnect. */
  configure(session: ProtectedSession, policy: unknown, sources: PreScriptPolicy): void {
    const key = JSON.stringify({ policy, sources });
    const existing = this.bindings.get(session);
    if (existing) return this.reuse(existing, key);
    if (protectionState(session).rendererStarted) throw Error('Native protection requires a cold session');
    const binding = { key, installed: false };
    this.bindings.set(session, binding);
    this.install(session, policy, sources);
    binding.installed = true;
  }
  /** Commit scalar and pre-script policy as one fail-closed installation attempt. */
  private install(session: ProtectedSession, policy: unknown, sources: PreScriptPolicy): void {
    this.policies.configure(session, policy);
    session._setOyaPreScriptPolicy!({ ...sources });
    verifyProtectionSources(session, sources);
  }
  /** Reject a changed persona or a failed partial installation without touching the engine. */
  private reuse(binding: ProtectionBinding, key: string): void {
    if (!binding.installed) throw Error('Native protection failed; restart Oya before reusing this session');
    if (binding.key !== key) throw Error('Native persona policy changed; restart Oya to apply it safely');
  }
  /** Every exposed surface must belong to a completely configured session. */
  assertConfigured(session: ProtectedSession): void {
    const binding = this.bindings.get(session);
    if (!binding?.installed) throw Error('Native session protection has not been installed');
  }
}
