/**
 * Counts what /downloads hands out: a person downloading an installer, an
 * installed app checking for an update (the latest*.yml feed), and the app
 * fetching the update itself. It runs in front of the static files and
 * counts once the response has gone out whole, so a missing file, a resumed
 * download's later ranges and a HEAD are not counted. Who asked is a
 * fingerprint of their address and user agent: enough to count unique
 * downloaders, never enough to name one. Where they came from is the oya_src
 * cookie the site keeps (an ad, a campaign, a linking site, or direct), else
 * the site that linked the file; a client that is not a browser is named, so
 * a crawler's burst of every platform reads as one.
 */
import type { NextFunction, Request, Response } from 'express';
import { fingerprint } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { track } from './service.ts';
import type { EventProps } from './catalog.ts';
import { UNKNOWN, VERSION_HEADER, versionOf } from './version.ts';
import { SOURCE_MAX_LENGTH } from './constants.ts';
import { countDownload } from '../admin/index.ts';

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
/** The cookie the site keeps a visitor's source in (ui/src/lib/visitor-source.ts). */
const SOURCE_COOKIE = 'oya_src';
/** What a kept source may look like; anything else is not printed. */
const SOURCE = new RegExp(`^[\\w.\\-/ ]{1,${SOURCE_MAX_LENGTH}}$`);
/** Clients that are not a person's browser: crawlers, link previews, scripts and AI agents. */
const NOT_A_BROWSER =
  /bot|crawl|spider|slurp|preview|scan|curl|wget|python|go-http|java\/|okhttp|axios|node-fetch|undici|headless|facebookexternalhit|claude|gpt|perplexity/i;

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
  if (Object.hasOwn(FEEDS, name)) return () => noteCheck(req, name);
  const match = RELEASE_FILE.exec(name);
  return match ? () => noteDownload(req, match[1], match[2]) : null;
}

/** An update check: counted for the admin page, and sent as an event. */
function noteCheck(req: Request, feed: string) {
  const props = updateCheck(req, feed);
  countDownload('update_check', props.platform);
  track.updateChecked(visitorOf(req), props);
}

/** A download: counted for the admin page as an installer a person fetched or an update the app did, and sent as an event. */
function noteDownload(req: Request, version: string, ext: string) {
  const props = download(req, version, ext);
  countDownload(props.via === 'updater' ? 'update' : props.file_type, props.platform);
  track.downloadServed(visitorOf(req), props);
}

/** What an update check says: which feed, and the version asking. */
const updateCheck = (req: Request, feed: string) => ({
  platform: FEEDS[feed],
  from_version: versionOf(req.headers[VERSION_HEADER]),
});

/** The raw value of the source cookie, or '' when the request has none. */
const sourceCookie = (req: Request) =>
  String(req.headers.cookie || '')
    .split(';')
    .map((c) => c.trim())
    .find((c) => c.startsWith(`${SOURCE_COOKIE}=`))
    ?.slice(SOURCE_COOKIE.length + 1) || '';

/** The source the site kept for this visitor, or '' when there is none or it is malformed. */
function keptSource(req: Request) {
  try {
    const source = decodeURIComponent(sourceCookie(req));
    return SOURCE.test(source) ? source : '';
  } catch {
    return '';
  }
}

/** The site that linked the file when it was not this one, or '' (a typed URL, or a client that sends no referrer). */
function linkingSite(req: Request) {
  try {
    const host = new URL(String(req.headers.referer || '')).host.replace(/^www\./, '');
    return host && host !== String(req.headers.host || '').replace(/^www\./, '') ? host : '';
  } catch {
    return '';
  }
}

/** Where the download came from: the kept source, else the linking site, else `direct`. */
const sourceOf = (req: Request) => keptSource(req) || linkingSite(req) || 'direct';

/** The client's name when it is not a person's browser, '' when it is one. */
function clientOf(req: Request) {
  const agent = String(req.headers['user-agent'] || '');
  if (agent.startsWith('Mozilla/') && !NOT_A_BROWSER.test(agent)) return '';
  return (agent.match(NOT_A_BROWSER)?.[0] || agent.split(' ')[0] || 'no user agent').slice(0, SOURCE_MAX_LENGTH);
}

/** Where a download came from, and the client when it is not a browser. */
const origin = (req: Request) => ({ source: sourceOf(req), client: clientOf(req) });

/**
 * What a download says: platform, release, installer or update, whether a
 * person or the updater asked, where they came from, and the client when it
 * is not a browser.
 */
function download(req: Request, version: string, ext: string): EventProps['download_served'] {
  const update = Object.hasOwn(UPDATES, ext);
  const platform = (update ? UPDATES[ext] : INSTALLERS[ext]) || UNKNOWN;
  const via = UPDATER_AGENT.test(String(req.headers['user-agent'] || '')) ? 'updater' : 'web';
  return { platform, version, file_type: update ? 'update' : 'installer', via, ...origin(req) };
}

/** Middleware for /downloads: counts the file once its response has finished, then lets the static files answer. */
export function trackDownloads(req: Request, res: Response, next: NextFunction) {
  const count = req.method === 'GET' ? eventFor(req) : null;
  if (count) res.once('finish', () => servedWhole(req, res) && count());
  next();
}
