/**
 * Counts what /downloads hands out: a person downloading an installer, an
 * installed app checking for an update (the latest*.yml feed), and the app
 * fetching the update itself. It runs in front of the static files and
 * counts once the response has gone out whole, so a missing file, a resumed
 * download's later ranges and a HEAD are not counted. Who asked is a
 * fingerprint of their address and user agent: enough to count unique
 * downloaders, never enough to name one.
 */
import type { NextFunction, Request, Response } from 'express';
import { fingerprint } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { track } from './service.ts';
import type { EventProps } from './catalog.ts';
import { UNKNOWN, VERSION_HEADER, versionOf } from './version.ts';

/** An installer file's extension → the platform it installs on. */
const INSTALLERS: Record<string, string> = { dmg: 'mac', exe: 'windows', AppImage: 'linux' };
/** An update archive's extension → its platform (Windows updates from its .exe, counted as an installer the updater fetched). */
const UPDATES: Record<string, string> = { zip: 'mac' };
/** An update feed's file name → the platform that reads it. */
const FEEDS: Record<string, string> = {
  'latest-mac.yml': 'mac',
  'latest.yml': 'windows',
  'latest-linux.yml': 'linux',
};
/** A release file's name: `Oya.Browser-<version>-<arch>.<ext>`. */
const RELEASE_FILE = /^Oya\.Browser-(\d+\.\d+\.\d+)-[\w.-]+?\.(dmg|exe|AppImage|zip)$/;
/** The user agents electron-updater fetches with. */
const UPDATER_AGENT = /electron-builder|electron-updater|Electron/i;

/** Who asked, as a fingerprint no one can turn back into an address. */
function visitorOf(req: Request) {
  const forwarded = String(req.headers['x-forwarded-for'] || '')
    .split(',')[0]
    .trim();
  return `dl-${fingerprint(`${forwarded || req.socket?.remoteAddress || ''}|${req.headers['user-agent'] || ''}`)}`;
}

/** Whether the whole file went out: a 200, or a range response that starts at the first byte. */
function servedWhole(req: Request, res: Response) {
  if (res.statusCode === Status.OK) return true;
  return res.statusCode === Status.PARTIAL_CONTENT && /^bytes=0-/.test(String(req.headers.range || ''));
}

/** The requested file's name, or '' for a path that is not valid percent-encoding (a static file would 400 it too). */
function fileName(req: Request) {
  try {
    return decodeURIComponent(req.path.replace(/^\//, ''));
  } catch {
    return '';
  }
}

/** The event a finished response is, or null when it is nothing worth counting. */
function eventFor(req: Request) {
  const name = fileName(req);
  if (Object.hasOwn(FEEDS, name)) return () => track.updateChecked(visitorOf(req), updateCheck(req, name));
  const match = RELEASE_FILE.exec(name);
  return match ? () => track.downloadServed(visitorOf(req), download(req, match[1], match[2])) : null;
}

/** What an update check says: which feed, and the version asking. */
const updateCheck = (req: Request, feed: string) => ({
  platform: FEEDS[feed],
  from_version: versionOf(req.headers[VERSION_HEADER]),
});

/** What a download says: platform, release, installer or update, and whether a person or the updater asked. */
function download(req: Request, version: string, ext: string): EventProps['download_served'] {
  const update = Object.hasOwn(UPDATES, ext);
  const platform = (update ? UPDATES[ext] : INSTALLERS[ext]) || UNKNOWN;
  const via = UPDATER_AGENT.test(String(req.headers['user-agent'] || '')) ? 'updater' : 'web';
  return { platform, version, file_type: update ? 'update' : 'installer', via };
}

/** Middleware for /downloads: counts the file once its response has finished, then lets the static files answer. */
export function trackDownloads(req: Request, res: Response, next: NextFunction) {
  const count = req.method === 'GET' ? eventFor(req) : null;
  if (count) res.once('finish', () => servedWhole(req, res) && count());
  next();
}
