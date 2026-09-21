/**
 * `oya.config`: this key's settings (LLM credentials, browser provider,
 * solver).
 */
import type { Config, ConfigUpdate } from '../types/index.js';
import type { HttpRef } from './shapes.js';

/** Builds `oya.config`. */
export const configApi = (http: HttpRef) => ({
  /** This key's settings. Secrets read back masked. */
  get: <T = Config>(): Promise<T> => http().request<T>('GET', '/api/config'),
  /** Change settings; `null` clears a field back to the deployment default. */
  set: <T = Config>(values: ConfigUpdate): Promise<T> => http().request<T>('POST', '/api/config', values),
});
