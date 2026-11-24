/**
 * Root-level machine-readable discovery routes (/llms.txt, /.well-known/llms.txt,
 * /openapi.json, /.well-known/agent.json and /agent-card.json) served with explicit
 * public caching while passing unknown /.well-known/* requests through to OAuth.
 */
import express from 'express';
import { join } from 'node:path';
import { MS_PER_SECOND } from '../platform/constants.ts';
import { DISCOVERY_CACHE_CONTROL, DISCOVERY_MAX_AGE_SECONDS } from './constants.ts';

/** Paths that all serve the single curated llms.txt index. */
const LLMS_PATHS = ['/llms.txt', '/llm.txt', '/docs.txt', '/.well-known/llms.txt'];

/** Static maxAge in milliseconds for /.well-known files. */
const WELL_KNOWN_MAX_AGE_MS = DISCOVERY_MAX_AGE_SECONDS * MS_PER_SECOND;

/** Sends a discovery file relative to root so dot-prefixed checkout paths never trigger 404 or need dotfiles:allow. */
function sendDiscovery(res: express.Response, root: string, fileName: string, contentType: string): void {
  res.setHeader('Cache-Control', DISCOVERY_CACHE_CONTROL);
  res.type(contentType).sendFile(fileName, { root, cacheControl: false });
}

/** Mounts /.well-known static files and the A2A agent-card alias, falling through on miss for OAuth. */
function mountWellKnown(router: express.Router, publicDir: string): void {
  const wellKnownDir = join(publicDir, '.well-known');
  /** OpenAI's plugin directory proves we own the domain by fetching this token, which must come back as plain text. */
  router.get('/.well-known/openai-apps-challenge', (_req, res) =>
    sendDiscovery(res, wellKnownDir, 'openai-apps-challenge', 'text/plain'),
  );
  /** Serves static /.well-known discovery files (e.g. agent.json) and falls through on miss. */
  router.use('/.well-known', express.static(wellKnownDir, { maxAge: WELL_KNOWN_MAX_AGE_MS }));
  /** Serves the A2A agent card under its renamed path. */
  router.get('/.well-known/agent-card.json', (_req, res) =>
    sendDiscovery(res, wellKnownDir, 'agent.json', 'application/json'),
  );
}

/** Express router for public AI/agent discovery documents under publicDir. */
export function discoveryRouter(publicDir: string): express.Router {
  const router = express.Router();
  mountWellKnown(router, publicDir);
  /** Serves the curated llms.txt index under all standard crawler aliases. */
  router.get(LLMS_PATHS, (_req, res) => sendDiscovery(res, publicDir, 'llms.txt', 'text/plain'));
  /** Serves the OpenAPI 3.0 specification. */
  router.get('/openapi.json', (_req, res) => sendDiscovery(res, publicDir, 'openapi.json', 'application/json'));
  return router;
}
