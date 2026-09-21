/**
 * Profile store, persistent profile management on disk.
 * Profiles stored as JSON in app userData directory.
 */

const fs = require('fs');
const path = require('path');
const { PROFILE_JSON_INDENT } = require('./constants');

/** Repository for anonymity profiles: one JSON file each, plus which one is active. */
class ProfileStore {
  /** Keeps profiles under `<basePath>/profiles`, creating the folder if needed. */
  constructor(basePath) {
    /** The profiles folder. */
    this.dir = path.join(basePath, 'profiles');
    this._ensureDir();
  }

  /** Creates the profiles folder on first use. */
  _ensureDir() {
    if (!fs.existsSync(this.dir)) {
      fs.mkdirSync(this.dir, { recursive: true });
    }
  }

  /** The file for a profile id; an id that could escape the folder is refused. */
  _profilePath(id) {
    if (!/^[a-zA-Z0-9_-]+$/.test(id)) throw new Error('Invalid profile id');
    return path.join(this.dir, `${id}.json`);
  }

  /** Writes a profile and returns it. */
  save(profile) {
    fs.writeFileSync(this._profilePath(profile.id), JSON.stringify(profile, null, PROFILE_JSON_INDENT));
    return profile;
  }

  /** A saved profile, or null when it is missing or unreadable. */
  get(id) {
    const p = this._profilePath(id);
    if (!fs.existsSync(p)) return null;
    try {
      return JSON.parse(fs.readFileSync(p, 'utf8'));
    } catch {
      return null;
    }
  }

  /** The id of the profile in use, or null when none has been chosen. */
  getActiveId() {
    const p = path.join(this.dir, 'active.json');
    try {
      const data = JSON.parse(fs.readFileSync(p, 'utf8'));
      return data.activeId || null;
    } catch {
      return null;
    }
  }

  /** Records which profile is in use. */
  setActiveId(id) {
    fs.writeFileSync(path.join(this.dir, 'active.json'), JSON.stringify({ activeId: id }));
  }
}

module.exports = { ProfileStore };
