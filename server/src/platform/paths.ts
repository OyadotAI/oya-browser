/**
 * Where the server finds things on disk. Everything is resolved from here, so a
 * module can move without its storage quietly moving with it.
 */
import { fileURLToPath } from 'url';
import { join } from 'path';

/** server/ */
export const SERVER_ROOT = fileURLToPath(new URL('../../', import.meta.url));
/** The repo's browser/ (analyzer, recording channel, persona injection). */
export const BROWSER_DIR = join(SERVER_ROOT, '..', 'browser');
/** The repo's ui/, the Next.js frontend. */
export const UI_DIR = join(SERVER_ROOT, '..', 'ui');
/** Discovery files served at the root: .well-known, llms.txt, openapi.json. */
export const PUBLIC_DIR = join(SERVER_ROOT, 'src', 'public');
/** Oya Browser installers, served under /downloads. */
export const DOWNLOADS_DIR = join(SERVER_ROOT, 'downloads');

/** Persistent state. OYA_DATA_DIR points tests (and deployments) elsewhere. */
export const dataPath = (...parts: string[]) => join(process.env.OYA_DATA_DIR || join(SERVER_ROOT, 'data'), ...parts);
