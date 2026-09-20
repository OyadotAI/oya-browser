/**
 * A timezone that does not match the exit IP's country is one of the cheapest
 * detections there is. Reported rather than enforced — the geo data needed to
 * fix it automatically is not something this server has.
 */
import { metrics } from '../../platform/metrics.ts';
import { COUNTRY_CODE_CHARS, PLAUSIBLE_REGIONS } from './constants.ts';

/** A coherence report; only `checked` when there was nothing to compare. */
export interface Coherence {
  /** Whether both a proxy geo and a known timezone region were available to compare. */
  checked: boolean;
  /** The timezone is plausible for the exit country. */
  ok?: boolean;
  /** The exit's two-letter country. */
  country?: string;
  /** The persona's reported timezone. */
  timezone?: string;
  /** What is wrong, when it is not ok. */
  detail?: string | null;
}

/** Whether the persona's timezone is plausible for its proxy's country; `checked: false` when it cannot be told. */
export function coherence(persona, fingerprint, proxy): Coherence {
  if (!proxy?.geo || !fingerprint?.timezone) return { checked: false };
  const country = String(proxy.geo).slice(0, COUNTRY_CODE_CHARS).toUpperCase();
  const zone = String(fingerprint.timezone);
  const plausible = PLAUSIBLE_REGIONS[country];
  if (!plausible) return { checked: false };
  const ok = plausible.includes(zone.split('/')[0]);
  if (!ok) metrics.proxyIncoherent.inc({});
  return verdict(persona, ok, country, zone);
}

/** The report for a checked persona, naming the mismatch when there is one. */
function verdict(persona, ok, country, zone): Coherence {
  return {
    checked: true,
    ok,
    country,
    timezone: zone,
    detail: ok ? null : `persona ${persona.id} reports ${zone} but exits in ${country}`,
  };
}
