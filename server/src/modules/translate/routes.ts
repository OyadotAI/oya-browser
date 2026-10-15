/**
 * REST route for page translation: checks the body, calls the service, answers.
 * Internal to the desktop app, so it is not in the public OpenAPI description.
 */
import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { enforce } from '../../platform/limits.ts';
import { Status } from '../../platform/http-status.ts';
import { getKey, requireBrowser } from '../../app/http.ts';
import { translateTexts } from './service.ts';
import { TRANSLATE_BATCH_CHARS, TRANSLATE_BATCH_SEGMENTS, TRANSLATE_TAG } from './constants.ts';

/** The translate route, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** The fields a translate body may carry; anything else is refused. */
const FIELDS = new Set(['texts', 'target', 'source']);

/** Whether `texts` is a non-empty array of strings within the batch limits. */
function validTexts(texts: unknown) {
  if (!Array.isArray(texts) || !texts.length || texts.length > TRANSLATE_BATCH_SEGMENTS) return false;
  if (!texts.every((t) => typeof t === 'string')) return false;
  return texts.reduce((n, t) => n + t.length, 0) <= TRANSLATE_BATCH_CHARS;
}

/** Whether a value is a language tag. */
const isTag = (v: unknown) => typeof v === 'string' && TRANSLATE_TAG.test(v);

/** Why a translate body is refused, or null. */
export function translateRefusal(body: any): string | null {
  if (!body || typeof body !== 'object' || Object.keys(body).some((k) => !FIELDS.has(k))) return 'Unknown field';
  if (!validTexts(body.texts))
    return `texts must be 1-${TRANSLATE_BATCH_SEGMENTS} strings, ${TRANSLATE_BATCH_CHARS} characters at most`;
  if (!isTag(body.target) || (body.source !== undefined && !isTag(body.source)))
    return 'target and source must be language tags';
  return null;
}

/** POST /browsers/:browserId/translate, one batch of a page's text into the person's language. */
router.post(
  '/browsers/:browserId/translate',
  authMiddleware,
  enforce('translate'),
  requireBrowser,
  async (req, res) => {
    const error = translateRefusal(req.body);
    if (error) return res.status(Status.BAD_REQUEST).json({ error, code: 'invalid_request' });
    res.json(
      await translateTexts(getKey(req), req.params.browserId, { texts: req.body.texts, target: req.body.target }),
    );
  },
);
