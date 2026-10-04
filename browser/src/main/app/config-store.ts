/**
 * The desktop's saved settings: config.json in the user-data folder, with
 * environment overrides for Docker and headless use. The one place the main
 * process reads or writes that file. The API key is a credential for the whole
 * project, so it is sealed with the OS keychain (Electron safeStorage) when one
 * is there; without one (Linux with no keyring) it stays in the owner-only file.
 */
import fs from 'node:fs';
import path from 'node:path';
import type { SafeStorage } from 'electron';
import { PRIVATE_FILE_MODE, JSON_INDENT, DEFAULT_SERVER_URL } from './constants.ts';

/** The shell's own choices, kept with the settings. */
export interface UiPreferences {
  /** 'system', 'light' or 'dark'. */
  theme?: string;
  /** The page format the agent reads pages in. */
  pageFormat?: string;
  /** The workspace panel's width. */
  panelWidth?: number;
  /** Anything else an older shell saved. */
  [key: string]: unknown;
}

/** The settings in effect: the saved file over the defaults, with the environment over both. */
export interface ConfigValues {
  /** The control socket's address. */
  serverUrl: string;
  /** The project key ('' when signed out). */
  apiKey: string;
  /** The name this browser goes by in the console. */
  browserName: string;
  /** The fingerprint profile last in use. */
  activeProfileId: string | null;
  /** The browser logins were last imported from. */
  mirroredFrom: string;
  /** The key as the keychain sealed it, kept when it could not be opened this launch. */
  apiKeySealed?: string;
  /** The persona this browser asks the server for. */
  persona?: string;
  /** Where this browser runs, when the deployment says. */
  provider?: string;
  /** Set by a log out, so an environment key does not sign it back in. */
  signedOut?: boolean;
  /** Set when the key and server were entered in the app, so they win over the environment. */
  keyFromApp?: boolean;
  /** Who this browser is signed in as, as the server said last. */
  account?: Record<string, unknown> | null;
  /** The persona's display name, as the server sent it. */
  profileName?: string;
  /** The shell's appearance and panel. */
  ui?: UiPreferences;
  /** Any other field the app keeps (routines from before the move, import records, sync marks). */
  [key: string]: unknown;
}

/** Electron's safeStorage, as far as sealing the key goes; null outside Electron. */
export type Sealer = Partial<
  Pick<SafeStorage, 'isEncryptionAvailable' | 'getSelectedStorageBackend' | 'encryptString' | 'decryptString'>
> | null;

/** What a ConfigStore is built with. */
export interface ConfigStoreDeps {
  /** The user-data folder, asked at load time. */
  dir: () => string;
  /** The environment that overrides the file; this process's by default. */
  env?: Record<string, string | undefined>;
  /** The platform the default browser name names; this process's by default. */
  platform?: string;
  /** The keychain the key is sealed with; none (the key stays in the owner-only file) by default. */
  safe?: Sealer;
}

/** Whether `safe` really encrypts: Linux without a keyring falls back to 'basic_text', which only obscures. */
const sealable = (safe: Sealer | undefined): boolean =>
  !!safe?.isEncryptionAvailable?.() && safe.getSelectedStorageBackend?.() !== 'basic_text';

/** What people call each platform; the name a browser gets is read by people, in the dialog and in the console's fleet. */
const PLATFORM_NAMES: Readonly<Record<string, string>> = { darwin: 'Mac', win32: 'Windows', linux: 'Linux' };

/** What a fresh install starts from; the saved file and the environment override it. */
function configDefaults(platform: string): ConfigValues {
  return {
    serverUrl: DEFAULT_SERVER_URL,
    apiKey: '',
    browserName: `Oya Browser on ${Object.hasOwn(PLATFORM_NAMES, platform) ? PLATFORM_NAMES[platform] : platform}`,
    activeProfileId: null,
    mirroredFrom: '',
  };
}

/** Sets one field from the environment. */
type Override = (config: ConfigValues, value: string) => unknown;

/**
 * Environment variable → config field, applied over the saved file. The key and
 * server give way to ones a person entered in the app (`keyFromApp`): a shell
 * that exports OYA_API_KEY for another Oya product replaced the key on every
 * launch, the server refused it, and the person had to enter theirs again.
 */
const ENV_OVERRIDES: Readonly<Record<string, Override>> = {
  OYA_SERVER_URL: (config, value) => config.keyFromApp || (config.serverUrl = value),
  OYA_API_KEY: (config, value) => config.keyFromApp || (config.apiKey = value.split(',')[0].trim()),
  OYA_BROWSER_NAME: (config, value) => (config.browserName = value),
  OYA_PERSONA: (config, value) => (config.persona = value),
  OYA_PROVIDER: (config, value) => (config.provider = value),
};

/** Reads and writes config.json; `values` is the live settings object everyone shares. */
export class ConfigStore {
  /** The settings in effect. */
  values: ConfigValues;
  /** config.json's path, once loaded. */
  file: string | null = null;
  /** The keychain the API key is sealed with, when it is available. */
  private readonly safe: Sealer;
  /** Where config.json lives, asked when it is loaded. */
  private readonly dir: () => string;
  /** The environment that overrides the file. */
  private readonly env: Record<string, string | undefined>;
  /** The defaults a load starts from. */
  private readonly defaults: ConfigValues;

  /** `deps` names the folder, and may replace the environment, platform and keychain. */
  constructor({ dir, env = process.env, platform = process.platform, safe = null }: ConfigStoreDeps) {
    this.safe = safe;
    this.dir = dir;
    this.env = env;
    this.values = configDefaults(platform);
    this.defaults = configDefaults(platform);
  }

  /** Reads config.json (a missing or broken file keeps what is there) and applies the environment. */
  load(): ConfigValues {
    this.file = path.join(this.dir(), 'config.json');
    try {
      this.unseal({ ...this.defaults, ...JSON.parse(fs.readFileSync(this.file, 'utf8')) });
    } catch {}
    this.applyEnvironment();
    // A person who logged out stays out, whatever key the environment carries.
    if (this.values.signedOut) this.values.apiKey = '';
    return this.values;
  }

  /** Applies the environment's overrides over the settings. */
  private applyEnvironment(): void {
    for (const [name, apply] of Object.entries(ENV_OVERRIDES)) {
      const value = this.env[name];
      if (value) apply(this.values, value);
    }
  }

  /**
   * Takes the saved settings with the sealed key opened. A key still in plain
   * text, from before sealing or from a machine that had no keychain, is sealed
   * now, before the environment can put its own key in the way.
   */
  private unseal(saved: ConfigValues): void {
    const { apiKeySealed, ...values } = saved;
    const opened = apiKeySealed && sealable(this.safe) ? this.openKey(apiKeySealed) : '';
    // Unopenable today (keychain locked, denied or not running) is not gone: keep it for a later launch.
    Object.assign(values, opened ? { apiKey: opened } : apiKeySealed && { apiKeySealed });
    this.values = values;
    if (!apiKeySealed && values.apiKey && sealable(this.safe)) this.save();
  }

  /** The key a sealed value holds; '' when this keychain cannot open it (a copied profile, a reset keychain). */
  private openKey(sealed: string): string {
    try {
      return this.safe?.decryptString?.(Buffer.from(sealed, 'base64')) ?? '';
    } catch {
      return '';
    }
  }

  /** What goes in the file: the settings, with the key sealed when the keychain can. */
  private sealed(): ConfigValues {
    const { apiKey, ...rest } = this.values;
    if (!apiKey || !sealable(this.safe) || !this.safe?.encryptString) return this.values;
    return { ...rest, apiKey: '', apiKeySealed: this.safe.encryptString(apiKey).toString('base64') };
  }

  /** Merges `changes` over the settings in effect. */
  merge(changes: Partial<ConfigValues>): void {
    this.values = { ...this.values, ...changes };
  }

  /** Writes the settings, owner-only. A failed write keeps the running settings. */
  save(): void {
    try {
      if (!this.file) throw new Error('config.json is not loaded yet');
      fs.writeFileSync(this.file, JSON.stringify(this.sealed(), null, JSON_INDENT), { mode: PRIVATE_FILE_MODE });
      fs.chmodSync(this.file, PRIVATE_FILE_MODE); // a file written before this was 0644
    } catch {}
  }
}
