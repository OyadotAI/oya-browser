/**
 * Stable download addresses: /download/mac and /download/windows always answer
 * with the newest installer, so instructions an agent follows (llms.txt, the
 * skill) never carry a version. They redirect into /downloads, where the file
 * is served and counted as before. Newest is found by scanning the downloads
 * folder, as the release workflow does; dev deploys ship no update manifests.
 */
import { readdir } from 'node:fs/promises';
import { Router, type Request, type Response } from 'express';
import { notFound, sendError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { DOWNLOADS_DIR } from '../../platform/paths.ts';

/** Each platform's installer, by the end of its file name. */
const SUFFIX: Record<string, string> = { mac: '-universal.dmg', windows: '-x64.exe' };
/** A release file's version, as electron-builder names it. */
const VERSION = /^Oya\.Browser-(\d+)\.(\d+)\.(\d+)-/;

/** The version triple in a release file's name, or null for any other file. */
const versionOf = (name: string) => VERSION.exec(name)?.slice(1).map(Number) ?? null;

/** Whether version a is newer than version b, compared part by part as numbers. */
function newer(a: number[], b: number[]) {
  const i = a.findIndex((part, at) => part !== b[at]);
  return i >= 0 && a[i] > b[i];
}

/** The files in `dir`, or none when it does not exist. */
const filesIn = (dir: string) => readdir(dir).catch(() => [] as string[]);

/** The newest installer for `platform` in `dir`, or null when there is none or the platform is unknown. */
export async function latestInstaller(dir: string, platform: string): Promise<string | null> {
  if (!Object.hasOwn(SUFFIX, platform)) return null;
  const installers = (await filesIn(dir)).filter((name) => name.endsWith(SUFFIX[platform]) && versionOf(name));
  return installers.reduce<string | null>((best, name) => (!best || newer(versionOf(name), versionOf(best)) ? name : best), null);
}

/** Redirects a request for a platform to its newest installer in `dir`, or answers 404. */
const redirectToLatest = (dir: string) => async (req: Request, res: Response) => {
  const name = await latestInstaller(dir, req.params.platform as string);
  if (!name) return sendError(res, notFound('installer'), req);
  res.redirect(Status.FOUND, `/downloads/${name}`);
};

/** The /download routes, reading installers from `dir` (the server's downloads folder unless a test passes its own). */
export function downloadRoutes(dir = DOWNLOADS_DIR) {
  return Router({ caseSensitive: true }).get('/:platform', redirectToLatest(dir));
}
