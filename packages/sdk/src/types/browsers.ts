/**
 * Types for browsers themselves: starting one, what a page looks like to the
 * SDK, the results of CAPTCHA and MFA handling, and the fleet listing.
 */

/** Where a browser comes from. Configuration, not something a caller must know. */
export type Provider = 'oya-cloud' | 'oya-selfhosted' | 'browseruse' | 'browserbase' | 'steel' | 'anchor' | 'cdp';

/** How `oya.browser.start()` should start a browser. Every field is optional. */
export interface StartOptions {
  /** Reuse this value when retrying the same logical creation. */
  idempotencyKey?: string;
  /** Wait for capacity for up to five minutes; zero rejects immediately. */
  queueMs?: number;
  /** Spend ceiling for this browser, in US dollars. */
  budgetUsd?: number;
  /** Run under the project's governance: policies, budgets and recordings. */
  governed?: boolean;
  /** Egress and recording rules for a governed browser. */
  policy?: {
    /** Hosts the browser may reach; everything else is blocked. */
    allowedHosts?: string[];
    /** Hosts where a person, not the agent, must act. */
    humanHosts?: string[];
    /** Where the browser must run. */
    region?: string;
    /** Blur typed values out of the session recording. */
    redactRecording?: boolean;
  };
  /** Place in the queue when capacity is short. */
  priority?: 'low' | 'normal' | 'high';
  /** Saved login profile. Defaults to the desktop's default profile. */
  profile?: string;
  /**
   * Which identity to run as. A persona is one device: fingerprint, cookie jar
   * and proxy bound together and stable for its life.
   *   'default', this key's own persona (the default)
   *   'auto'   , the least recently used persona under its concurrency cap
   *   <id>     , a specific persona
   */
  persona?: 'default' | 'auto' | (string & {});
  /** Solve CAPTCHAs as they appear rather than waiting to be asked. */
  captcha?: 'auto' | 'off';
  /** Override the key's configured provider for this browser only. */
  provider?: Provider;
  /**
   * Required only for the 'cdp' provider: the CDP WebSocket URL, or the plain
   * `http://localhost:9222` Chrome prints for `--remote-debugging-port`.
   */
  wsUrl?: string;
  /** A label for the fleet listing. */
  name?: string;
  /** How long to wait for a cloud browser to dial in. Default 120s. */
  readyTimeoutMs?: number;
}

/** What the server answers when a browser starts. */
export interface StartResult {
  /** The browser's id, for every later call. */
  id: string;
  /** Which provider runs it. */
  provider: string;
  /** Which persona it runs as. */
  persona: string;
  /** 'starting' while a cloud browser has yet to dial in. */
  status: 'ready' | 'starting';
  /** Point Playwright, Puppeteer or browser-use at this. */
  cdpUrl?: string;
  /** Anything the server wants the caller to know about this start. */
  note?: string;
  /** True when this browser was already running and was handed over rather than started. */
  reused?: boolean;
}

/** One element on the page, numbered by `analyze()`. */
export interface Element {
  /** The number to pass to `click()` and `type()`. Valid until the page changes. */
  id: number;
  /** What kind of control it is: link, button, input and so on. */
  type: string;
  /** Visible label, capped at 80 characters by the analyzer. */
  text?: string;
  /** Where a link goes. */
  href?: string;
  /** The current value of a field. */
  value?: string;
  /** Whether a checkbox or radio is ticked. */
  checked?: boolean;
  /** Whether it refuses input. */
  disabled?: boolean;
  /** Whether it is on screen. */
  visible: boolean;
  /** The HTML tag name. */
  tag?: string;
  /** The element's DOM `id`. Usually the most stable handle a site offers. */
  domId?: string;
  /** Its `aria-label`. */
  ariaLabel?: string;
  /** `data-testid`, when the site ships one. */
  testId?: string;
  /** Its `name` attribute. */
  name?: string;
  /** A field's placeholder text. */
  placeholder?: string;
  /** Action of the enclosing form. */
  formName?: string;
}

/** How `analyze()` writes the page: markdown (the default) or toon (toonformat.dev). */
export type PageFormat = 'markdown' | 'toon' | 'jsonl';

/** What `analyze()` accepts. */
export interface AnalyzeOptions {
  /** How the page is written: markdown (the default) or toon. */
  format?: PageFormat;
}

/** One block of a page, in reading order: a heading, paragraph, list item, table row, image or element. */
export interface Block {
  /** The element's id, for interactive elements; act on it with click and type. */
  id?: number;
  /** The part of the page it is in: nav, main, form/…, dialog, or empty. */
  region: string;
  /** What it is: h1-h6, text, item, row, header, quote, code, image, or an element kind (link, button, input:email…). */
  kind: string;
  /** Its text, or an element's name. */
  text?: string;
  /** Where a link goes, or what a field holds now. */
  target?: string;
  /** An element's state: checked, disabled, required, off-screen, hint… */
  state?: string;
}

/** The page as `analyze()` sees it. */
export interface Analysis {
  /** The format `page` is written in. */
  format?: PageFormat;
  /** The page, written in `format`. */
  page?: string;
  /** The page as markdown; present when the format is markdown (the default). */
  markdown?: string;
  /** The page's facts: url, title, scroll, and when they apply panelScroll, modal, covered, truncated. */
  facts?: Record<string, string | number>;
  /** The page as data: every block in reading order, whatever the format. */
  blocks?: Block[];
  /** Every numbered element, visible or not. */
  elements: Element[];
  /** The window size, in CSS pixels. */
  viewport?: {
    /** Width in CSS pixels. */
    width: number;
    /** Height in CSS pixels. */
    height: number;
  };
  /** How far the page is scrolled. */
  scroll?: {
    /** Horizontal offset in CSS pixels. */
    x: number;
    /** Vertical offset in CSS pixels. */
    y: number;
  };
  /** True when the page was too long to send in full. */
  truncated?: boolean;
}

/** What `solveCaptcha()` found and did. */
export interface CaptchaResult {
  /** Whether a CAPTCHA was on the page. */
  present: boolean;
  /** Whether it was cleared. */
  solved: boolean;
  /** 'provider' when the vendor solved it, 'solver' when we did, 'none' otherwise. */
  method: 'provider' | 'solver' | 'none';
  /** Which kind of CAPTCHA it was. */
  type?: string;
  /** Invisible reCAPTCHA v3 scores the visit passively; there is nothing on screen to clear. */
  invisible?: boolean;
  /** The site key the solver was given. */
  sitekey?: string | null;
  /** Why it was not solved. */
  error?: string;
}

/** What `completeMfa()` found and did. */
export interface MfaResult {
  /** Whether an MFA prompt was on the page. */
  present: boolean;
  /** Whether it was answered and accepted. */
  completed: boolean;
  /** Which factor answered it, or 'handoff' when a person must. */
  method?: 'totp' | 'email' | 'sms' | 'handoff' | 'none';
  /** Whether the code was typed in. */
  filled?: boolean;
  /** Whether the form was submitted. */
  submitted?: boolean;
  /** Open this to finish by hand when nothing automated can. */
  liveViewUrl?: string | null;
  /** Why it could not be completed. */
  error?: string;
}

/** How a browser is doing, judged from its recent commands and heartbeats. */
export type Health = 'ok' | 'stale' | 'errors' | 'dead';

/** One thing a browser did. */
export interface Activity {
  /** When, as an ISO timestamp. */
  ts: string;
  /** The command's name. */
  action: string;
  /** A one-line account of it. */
  summary: string;
  /** Whether it succeeded. */
  ok: boolean;
  /** How long it took. */
  ms: number;
  /** Why it failed. */
  error?: string;
}

/** The outcome of stopping one browser. */
export interface StopResult {
  /** The browser. */
  id: string;
  /** Whether it stopped. */
  ok: boolean;
  /** Which provider ran it. */
  provider?: string | null;
  /** For a cloud browser, whether its sandbox was destroyed (and billing ended). */
  sandboxRemoved?: boolean | null;
  /** Why it did not stop. */
  error?: string;
  /** True when nothing was stopped because the browser was only borrowed. */
  reused?: boolean;
}

/** One browser in the fleet listing. */
export interface BrowserInfo {
  /** The browser's id. */
  id: string;
  /** Its label. */
  name: string;
  /** An Oya Browser on the control socket, or a third-party browser over CDP. */
  clientType: 'oya' | 'cdp';
  /** Which provider runs it. */
  provider: string | null;
  /** The persona id it runs as. */
  persona: string | null;
  /** That persona's name. */
  personaName: string | null;
  /** How it is doing. */
  health: Health;
  /** When it connected. */
  connectedAt: string;
  /** When it was last heard from. */
  lastSeen: string;
  /** The page it is on. */
  currentUrl: string;
  /** Commands run so far. */
  commands: number;
  /** Commands that failed. */
  errors: number;
  /** Commands in flight. */
  pending: number;
  /** When it last ran a command. */
  lastCommandAt: string | null;
  /** The last failure's message. */
  lastError: string | null;
}

/** A browser plus what it has been doing, from `browser.status()`. */
export interface BrowserDetail extends BrowserInfo {
  /** Its most recent commands, newest first. */
  activity: Activity[];
}
