/**
 * The desktop's saved settings: config.json in the user-data folder, with
 * environment overrides for Docker and headless use. The one place the main
 * process reads or writes that file.
 */
const fs = require('fs');
const path = require('path');
const { PRIVATE_FILE_MODE, JSON_INDENT, DEFAULT_SERVER_URL } = require('./constants.cjs');

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

/** Environment variable → config field, applied over the saved file. */
const ENV_OVERRIDES = {
  OYA_SERVER_URL: (config, value) => (config.serverUrl = value),
  OYA_API_KEY: (config, value) => (config.apiKey = value.split(',')[0].trim()),
  OYA_BROWSER_NAME: (config, value) => (config.browserName = value),
  OYA_PERSONA: (config, value) => (config.persona = value),
  OYA_PROVIDER: (config, value) => (config.provider = value),
};

/** Reads and writes config.json; `values` is the live settings object everyone shares. */
class ConfigStore {
  /** `dir()` is the user-data folder, asked at load time; `env` and `platform` default to this process's. */
  constructor({ dir, env = process.env, platform = process.platform }) {
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
      this.values = { ...this.defaults, ...JSON.parse(fs.readFileSync(this.file, 'utf8')) };
    } catch {}
    for (const [name, apply] of Object.entries(ENV_OVERRIDES)) if (this.env[name]) apply(this.values, this.env[name]);
    // A person who logged out stays out, whatever key the environment carries.
    if (this.values.signedOut) this.values.apiKey = '';
    return this.values;
  }

  /** Merges `changes` over the settings in effect. */
  merge(changes) {
    this.values = { ...this.values, ...changes };
  }

  /** Writes the settings, owner-only. A failed write keeps the running settings. */
  save() {
    try {
      fs.writeFileSync(this.file, JSON.stringify(this.values, null, JSON_INDENT), { mode: PRIVATE_FILE_MODE });
      fs.chmodSync(this.file, PRIVATE_FILE_MODE); // a file written before this was 0644
    } catch {}
  }
}

module.exports = { ConfigStore };
