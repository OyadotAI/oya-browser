/**
 * The desktop's saved settings: config.json in the user-data folder, with
 * environment overrides for Docker and headless use. The one place the main
 * process reads or writes that file. The API key is a credential for the whole
 * project, so it is sealed with the OS keychain (Electron safeStorage) when one
 * is there; without one (Linux with no keyring) it stays in the owner-only file.
 */
const fs = require('fs');
const path = require('path');
const { PRIVATE_FILE_MODE, JSON_INDENT, DEFAULT_SERVER_URL } = require('./constants.cjs');

/** Electron's safeStorage, or null outside Electron (plain Node, as in unit tests). */
function electronSafeStorage() {
  try {
    return require('electron').safeStorage || null;
  } catch {
    return null;
  }
}

/** Whether `safe` really encrypts: Linux without a keyring falls back to 'basic_text', which only obscures. */
const sealable = (safe) => !!safe?.isEncryptionAvailable?.() && safe.getSelectedStorageBackend?.() !== 'basic_text';

/** What people call each platform; the name a browser gets is read by people, in the dialog and in the console's fleet. */
const PLATFORM_NAMES = { darwin: 'Mac', win32: 'Windows', linux: 'Linux' };

/** What a fresh install starts from; the saved file and the environment override it. */
function configDefaults(platform) {
  return {
    serverUrl: DEFAULT_SERVER_URL,
    apiKey: '',
    browserName: `Oya Browser on ${Object.hasOwn(PLATFORM_NAMES, platform) ? PLATFORM_NAMES[platform] : platform}`,
    activeProfileId: null,
    mirroredFrom: '',
  };
}

/**
 * Environment variable → config field, applied over the saved file. The key and
 * server give way to ones a person entered in the app (`keyFromApp`): a shell
 * that exports OYA_API_KEY for another Oya product replaced the key on every
 * launch, the server refused it, and the person had to enter theirs again.
 */
const ENV_OVERRIDES = {
  OYA_SERVER_URL: (config, value) => config.keyFromApp || (config.serverUrl = value),
  OYA_API_KEY: (config, value) => config.keyFromApp || (config.apiKey = value.split(',')[0].trim()),
  OYA_BROWSER_NAME: (config, value) => (config.browserName = value),
  OYA_PERSONA: (config, value) => (config.persona = value),
  OYA_PROVIDER: (config, value) => (config.provider = value),
};

/** Reads and writes config.json; `values` is the live settings object everyone shares. */
class ConfigStore {
  /**
   * `dir()` is the user-data folder, asked at load time; `env` and `platform`
   * default to this process's; `safe` is safeStorage (or a stand-in with its calls).
   */
  constructor({ dir, env = process.env, platform = process.platform, safe = electronSafeStorage() }) {
    /** The keychain the API key is sealed with, when it is available. */
    this.safe = safe;
    /** Where config.json lives, asked when it is loaded. */
    this.dir = dir;
    /** The environment that overrides the file. */
    this.env = env;
    /** The settings in effect. */
    this.values = configDefaults(platform);
    /** config.json's path, once loaded. */
    this.file = null;
    /** The defaults a load starts from. */
    this.defaults = configDefaults(platform);
  }

  /** Reads config.json (a missing or broken file keeps what is there) and applies the environment. */
  load() {
    this.file = path.join(this.dir(), 'config.json');
    try {
      this.unseal({ ...this.defaults, ...JSON.parse(fs.readFileSync(this.file, 'utf8')) });
    } catch {}
    for (const [name, apply] of Object.entries(ENV_OVERRIDES)) if (this.env[name]) apply(this.values, this.env[name]);
    // A person who logged out stays out, whatever key the environment carries.
    if (this.values.signedOut) this.values.apiKey = '';
    return this.values;
  }

  /**
   * Takes the saved settings with the sealed key opened. A key still in plain
   * text, from before sealing or from a machine that had no keychain, is sealed
   * now, before the environment can put its own key in the way.
   */
  unseal(saved) {
    const { apiKeySealed, ...values } = saved;
    const opened = apiKeySealed && sealable(this.safe) ? this.openKey(apiKeySealed) : '';
    // Unopenable today (keychain locked, denied or not running) is not gone: keep it for a later launch.
    Object.assign(values, opened ? { apiKey: opened } : apiKeySealed && { apiKeySealed });
    this.values = values;
    if (!apiKeySealed && values.apiKey && sealable(this.safe)) this.save();
  }

  /** The key a sealed value holds; '' when this keychain cannot open it (a copied profile, a reset keychain). */
  openKey(sealed) {
    try {
      return this.safe.decryptString(Buffer.from(sealed, 'base64'));
    } catch {
      return '';
    }
  }

  /** What goes in the file: the settings, with the key sealed when the keychain can. */
  sealed() {
    const { apiKey, ...rest } = this.values;
    if (!apiKey || !sealable(this.safe)) return this.values;
    return { ...rest, apiKey: '', apiKeySealed: this.safe.encryptString(apiKey).toString('base64') };
  }

  /** Merges `changes` over the settings in effect. */
  merge(changes) {
    this.values = { ...this.values, ...changes };
  }

  /** Writes the settings, owner-only. A failed write keeps the running settings. */
  save() {
    try {
      fs.writeFileSync(this.file, JSON.stringify(this.sealed(), null, JSON_INDENT), { mode: PRIVATE_FILE_MODE });
      fs.chmodSync(this.file, PRIVATE_FILE_MODE); // a file written before this was 0644
    } catch {}
  }
}

module.exports = { ConfigStore };
