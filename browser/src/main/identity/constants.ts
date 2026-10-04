/**
 * The fixed tables a persona's browser identity is built from: Chrome's
 * platforms, brands and client hints, and what a page gets without asking.
 */

/** How one navigator.platform looks: `os` in the UA string, `hints` in navigator.userAgentData. */
export interface PlatformLook {
  /** The parenthesised OS part of the UA string. */
  os: string;
  /** The platform fields of the user-agent metadata. */
  hints: PlatformHints;
}

/** The platform fields of navigator.userAgentData. */
export interface PlatformHints {
  /** The OS name. */
  platform: string;
  /** The OS version. */
  platformVersion: string;
}

/** Chrome version assumed when the session's user agent names none. */
export const FALLBACK_CHROME_VERSION = '134.0.0.0';

/** What each navigator.platform looks like: `os` in the UA string, `hints` in navigator.userAgentData. */
export const PLATFORMS: Readonly<Record<string, PlatformLook>> = {
  Win32: { os: 'Windows NT 10.0; Win64; x64', hints: { platform: 'Windows', platformVersion: '15.0.0' } },
  MacIntel: { os: 'Macintosh; Intel Mac OS X 10_15_7', hints: { platform: 'macOS', platformVersion: '14.6.1' } },
  'Linux x86_64': { os: 'X11; Linux x86_64', hints: { platform: 'Linux', platformVersion: '' } },
};

/** The navigator.platform of this machine, for a browser with no persona yet. */
export const HOST_PLATFORMS: Readonly<Record<string, string>> = { darwin: 'MacIntel', win32: 'Win32' };

/** The platform presented on any other host. */
export const DEFAULT_PLATFORM = 'Linux x86_64';

/** Chromium's GREASE alphabet, versions and brand orders (components/embedder_support/user_agent_utils.cc). */
export const GREASE_CHARS: readonly string[] = [' ', '(', ':', '-', '.', '/', ')', ';', '=', '?', '_'];
/** The versions a GREASE brand takes. */
export const GREASE_VERSIONS: readonly string[] = ['8', '99', '24'];
/** The slots the GREASE, Chromium and Google Chrome brands take, in that order, by `major % 6`. */
export const BRAND_ORDERS: readonly string[] = ['012', '021', '102', '120', '201', '210'];

/** Client-hint headers that are a quoted string, and the metadata field each one says. */
export const QUOTED_HINTS = {
  'sec-ch-ua-full-version': 'fullVersion',
  'sec-ch-ua-platform': 'platform',
  'sec-ch-ua-platform-version': 'platformVersion',
  'sec-ch-ua-arch': 'architecture',
  'sec-ch-ua-bitness': 'bitness',
  'sec-ch-ua-model': 'model',
} as const;

/** Client-hint headers that are a boolean: a desktop browser is neither mobile nor WOW64. */
export const BOOLEAN_HINTS = { 'sec-ch-ua-mobile': '?0', 'sec-ch-ua-wow64': '?0' } as const;

/**
 * Where Chrome puts the first request headers, measured off the wire (tls.peet.ws) on
 * Chrome 153; Electron writes them in its own order, and the hint headers are ours. A
 * navigation leads with the three hints; a fetch or subresource interleaves them with
 * the user agent. Both real Chromes measured start a fetch with sec-ch-ua-platform, and
 * servers read header order before any script runs. Headers not named keep their order.
 */
export const LEADING = {
  navigation: ['sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform', 'accept-language'],
  subresource: ['sec-ch-ua-platform', 'accept-language', 'sec-ch-ua', 'user-agent', 'sec-ch-ua-mobile'],
} as const satisfies Record<string, readonly string[]>;

/** Resource types Chrome sends as a navigation. */
export const NAVIGATIONS: ReadonlySet<string> = new Set(['mainFrame', 'subFrame']);

/** The hints every secure request carries, in Chrome's order. */
export const ALWAYS: readonly string[] = ['sec-ch-ua', 'sec-ch-ua-mobile', 'sec-ch-ua-platform'];

/** Every hint, in the order Chrome writes them when all are asked for. */
export const HINT_ORDER: readonly string[] = [
  'sec-ch-ua',
  'sec-ch-ua-mobile',
  'sec-ch-ua-full-version',
  'sec-ch-ua-arch',
  'sec-ch-ua-platform',
  'sec-ch-ua-platform-version',
  'sec-ch-ua-model',
  'sec-ch-ua-bitness',
  'sec-ch-ua-wow64',
  'sec-ch-ua-full-version-list',
];

/** Permissions Chrome grants without asking, in Electron's names. */
export const UNASKED: readonly string[] = [
  'fullscreen',
  'pointerLock',
  'keyboardLock',
  'clipboard-sanitized-write',
  'background-sync',
  'sensors',
];

/**
 * Asked, through a proxied persona's own session, where its traffic comes out: any
 * service answering JSON with an IANA `timezone` field (ipinfo.io does). The exit's
 * IP is all it sees, never this machine's.
 */
export const EXIT_GEO_URL = process.env.OYA_EXIT_GEO_URL || 'https://ipinfo.io/json';
/** How long a session waits for that answer before keeping the persona's own zone. */
export const EXIT_GEO_TIMEOUT_MS = 4000;

/** The page a governed browser may always show: it makes no request. */
export const BLANK_PAGE = 'about:blank';
/** The only schemes a governed browser's requests may use. */
export const EGRESS_PROTOCOLS: readonly string[] = ['http:', 'https:', 'ws:', 'wss:'];
