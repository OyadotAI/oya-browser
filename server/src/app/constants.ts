/**
 * Every number the HTTP app and the entry point run on, by name: ports, request
 * limits, keepalives and close codes.
 */
import { BYTES_PER_MIB } from '../platform/constants.ts';

/** Port the API listens on when PORT is unset. */
export const DEFAULT_PORT = 3100;
/** Radix for parsing PORT. */
export const DECIMAL = 10;
/** Loopback host the Next.js frontend listens on. */
export const LOOPBACK = '127.0.0.1';

/** Size of one HTTP status class: 404 / 100 is the 4xx class. */
export const STATUS_CLASS_SIZE = 100;

/** Mirrors MAX_FILE_BYTES in the SDK's file(), in MiB. */
const MAX_FILE_MIB = 10;
/** Mirrors MAX_FILE_BYTES in the SDK's file(); express.json()'s limit is sized for it. */
export const MAX_FILE_BYTES = MAX_FILE_MIB * BYTES_PER_MIB;
/** Longest task file name accepted. */
export const MAX_FILE_NAME_CHARS = 255;
/** Longest task file MIME type accepted. */
export const MAX_FILE_TYPE_CHARS = 128;
/** Bytes base64 encodes per group. */
export const BASE64_GROUP_BYTES = 3;
/** Characters base64 writes per group. */
export const BASE64_GROUP_CHARS = 4;

/** How often a long JSON answer trickles whitespace to keep the connection open. */
export const LONG_JSON_KEEPALIVE_MS = 15_000;

/**
 * Longest a client may take to send a request's headers: a slow-loris guard.
 * Upgrades (/ws, /connect) are past it once their headers are in.
 */
export const HEADERS_TIMEOUT_MS = 60_000;
/**
 * Longest a client may take to send a whole request, headers and body (15 MB
 * at most). Only the request is timed: a long answer (longJson, SSE, a
 * recording download) runs as long as it needs.
 */
export const REQUEST_TIMEOUT_MS = 300_000;

/**
 * Origins a web page may call the API from, comma separated in
 * OYA_CORS_ORIGINS. Unset, any origin may, but never with cookies: that is
 * what SDK clients holding an API key need, and the console is same-origin.
 */
export const CORS_ORIGIN: string | string[] = process.env.OYA_CORS_ORIGINS?.split(',').map((o) => o.trim()) ?? '*';

/** Close code for a browser socket presenting an invalid API key; the client stops reconnecting. */
export const INVALID_KEY_CLOSE_CODE = 4003;

/** rawHeaders entries per header: its name, then its value. */
export const RAW_HEADER_STRIDE = 2;

/** Where a caller who hit no route is pointed. */
export const API_DESCRIBED = 'The API is described at /openapi.json.';
