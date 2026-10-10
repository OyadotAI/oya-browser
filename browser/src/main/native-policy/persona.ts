/** Derive the enforceable identity subset from a real persona without host defaults or caller-supplied UA claims. */
import { personaIdentity, type PersonaProfile } from '../identity/identity.ts';
import { NATIVE_PLATFORMS } from './constants.ts';
import { validatePolicy } from './validate.ts';
import type { NativePolicy } from './types.ts';

/** Require structured persona input instead of quietly substituting this computer's identity. */
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Native persona requires an object');
  return value as Record<string, unknown>;
}
/** Only explicit supported platforms may reach the shared identity builder. */
function navigatorOf(value: unknown): Record<string, unknown> {
  const navigator = object(value);
  if (!NATIVE_PLATFORMS.includes(navigator.platform as never))
    throw Error('Native persona requires a supported platform');
  return navigator;
}
/** Native metadata uses the running engine version; absent engine evidence is never a fixed fallback. */
function engineUserAgent(value: unknown): string {
  const version =
    process.versions.chrome ||
    (typeof value === 'string' ? value.match(/Chrome\/(\d+\.\d+\.\d+\.\d+)(?:\s|$)/)?.[1] : '');
  if (!version || !/^[1-9]\d*\.\d+\.\d+\.\d+$/.test(version))
    throw Error('Native persona requires the actual engine Chrome version');
  return `Chrome/${version}`;
}
/** Reject malformed GPU descriptions before deriving Apple Silicon architecture. */
function webglOf(value: unknown): PersonaProfile['webgl'] {
  if (value === undefined) return undefined;
  const gpu = object(value);
  for (const name of ['renderer', 'unmaskedRenderer']) {
    if (gpu[name] !== undefined && typeof gpu[name] !== 'string') throw Error('Invalid native persona GPU identity');
  }
  return { renderer: gpu.renderer as string | undefined, unmaskedRenderer: gpu.unmaskedRenderer as string | undefined };
}
/** Support the engine's primary fallback only; never discard requested additional languages. */
function nativePersonaLanguages(value: unknown): unknown {
  if (!Array.isArray(value) || value.length !== 1 || typeof value[0] !== 'string') return value;
  const locale = Intl.getCanonicalLocales(value[0])[0];
  const primary = new Intl.Locale(locale).language;
  return locale === primary ? [locale] : [locale, primary];
}
/** Read locale and hardware without defaults so validation can reject incomplete personas. */
function settingsOf(persona: Record<string, unknown>, navigator: Record<string, unknown>) {
  return {
    timeZone: persona.timezone,
    locale: persona.locale,
    hardwareConcurrency: navigator.hardwareConcurrency,
    languages: nativePersonaLanguages(navigator.languages),
  };
}
/** Build only the native subset; all other persona protections remain mandatory before exposure. */
export function nativePolicyForPersona(value: unknown, actualEngineUserAgent?: string): NativePolicy {
  const persona = object(value),
    navigator = navigatorOf(persona.navigator);
  const profile = { navigator: { platform: navigator.platform as string }, webgl: webglOf(persona.webgl) };
  const identity = personaIdentity(profile, engineUserAgent(actualEngineUserAgent)).override;
  const userAgentMetadata = { ...identity.userAgentMetadata, formFactors: ['Desktop'] };
  return validatePolicy({ ...settingsOf(persona, navigator), ...identity, userAgentMetadata });
}

/** Before authentication, native host architecture must agree with the real unmodified device. */
export function nativePolicyForHost(value: unknown, actualEngineUserAgent?: string): NativePolicy {
  const policy = nativePolicyForPersona(value, actualEngineUserAgent);
  const architecture = process.arch === 'arm64' ? 'arm' : 'x86';
  return validatePolicy({ ...policy, userAgentMetadata: { ...policy.userAgentMetadata, architecture } });
}
