/** Fixed diagnostic channels grant authority only to the browser process, never a page world. */
export const BROWSER_DEBUG = {
  REGISTER: 'oya-debug-document',
  REQUEST: 'oya-debug-request',
  RESPONSE: 'oya-debug-response',
  MESSAGE: 'oya-debug-owner',
  MAX_EXPRESSION: 32768,
  MAX_JSON: 65536,
  MAX_REPLY_BYTES: 262144,
  MAX_URL: 4096,
  MAX_FRAMES: 128,
  MAX_DEPTH: 16,
  MAX_ELEMENTS: 300,
  MAX_TEXT: 12000,
  REQUEST_MS: 1500,
  EVAL_MS: 5000,
  OWNER_MS: 1000,
  OPERATION_MS: 13000,
  MAX_PENDING: 128,
  MAX_LABEL: 120,
  MAX_OPTIONS: 8,
  PROFILE_ID_CHARS: 20,
  MAX_ERROR: 1000,
  TOKEN_WORDS: 4,
  CENTRE: 2,
  MAX_ELEMENT_ID: 4294967295,
  LINE_RESERVE: 32,
  UTF8_MASK: 192,
  UTF8_CONTINUATION: 128,
} as const;

/** Requests are sent by native main to one exact preload document. */
export interface DebugRequest {
  /** Unique browser-generated correlation. */ id: string;
  /** Expected preload document; omission is admitted only for initial metadata. */ document?: string;
  /** Fixed trusted operation, never a source string interpreted in preload. */ kind: string;
  /** Checked operation input. */ params?: Record<string, unknown>;
}

/** Every reply identifies the actual native sender document. */
export interface DebugResponse {
  /** Correlates the exact request. */ id: string;
  /** Unforgeable outside the isolated preload. */ document: string;
  /** Explicit success. */ ok: boolean;
  /** JSON-safe result. */ data?: unknown;
  /** Bounded error message. */ error?: string;
}
