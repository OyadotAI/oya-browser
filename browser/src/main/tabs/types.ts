/**
 * The shapes the tab code shares: a tab as the manager lists it, its view
 * with the marks protection leaves on it, and the strip the collaborators
 * (tab order, popups, favicons, the start page) act on.
 */
import type { BrowserWindow } from 'electron';
import type { PageView } from '../cdp/cdp.ts';
import type { TabTranslation } from '../../shared/ipc.ts';

/** Where a tab's protection stands: still being set up, done, or given up on (nothing loads then). */
export type ProtectionState = 'pending' | 'protected' | 'failed';

/** One attempt at protecting a view; a retry fences the old one off. */
export interface Attempt {
  /** False once a retry replaced this attempt: it then sends and hears nothing. */
  live: boolean;
  /** Set by a critical step that failed. */
  failed: boolean;
}

/** A tab's view, with the marks protection keeps on it so a view is set up once per attempt. */
export interface TabView extends PageView {
  /** Set while an attempt owns the view (or once it finished). */
  oyaConfigured?: boolean;
  /** The current attempt's answer: whether the view is protected. */
  oyaSetup?: Promise<boolean>;
  /** The current attempt, fenced off on a reset. */
  oyaAttempt?: Attempt;
  /** Set once the isolated world is rebuilt on every load. */
  oyaWorldWired?: boolean;
}

/** One tab on the strip: a view in the persona's partition, or a window the page opened. */
export interface Tab {
  /** The tab's number. */
  id: number;
  /** What shows it, and whose webContents a command drives. */
  view: TabView;
  /** Its title on the strip. */
  title: string;
  /** Its address ('' on the start page). */
  url: string;
  /** Whether it is on the Oya start page (the shell draws it, no view is shown). */
  home?: boolean;
  /** Local collection drawn by the shell, never inside the website view. */
  collection?: import('../../shared/library.ts').LibraryKind;
  /** Profile that opened the private collection. */
  libraryPartition?: string;
  /** The real window, for a popup the page opened. */
  window?: BrowserWindow;
  /** Settles once protection settled (protected or given up on). */
  setup?: Promise<unknown>;
  /** Settles once the first page loaded (or failed to). */
  ready?: Promise<unknown>;
  /** Where its protection stands. */
  protection?: ProtectionState;
  /** Counts navigations, so a superseded one knows to stop. */
  navigationRequest?: number;
  /** Whether the address bar is waiting to load in it. */
  navigationPending?: boolean;
  /** Why its page failed, shown on the tab; null when it did not. */
  loadError?: string | null;
  /** Its icon as a data: URL, or null. */
  favicon?: string | null;
  /** The icon address last asked for. */
  faviconUrl?: string | null;
  /** The site the icon belongs to. */
  faviconOrigin?: string;
  /** Set when its own blank page committed, so the next dom-ready is that page. */
  blankCommitted?: boolean;
  /** The tab that opened it, so a test run's tabs close with it. */
  openerId?: number;
  /** Pinned to the strip's leading block: drawn as its icon, spared by bulk closes, restored pinned. */
  pinned?: boolean;
  /** What the address bar's translate chip shows for this page; main/translate owns it. */
  translation?: TabTranslation;
  /** The page's declared language tag, '' when it declared none (main/translate). */
  pageLang?: string;
  /** What this tab keeps translating on its own after the person said yes once; never saved. */
  translateKeep?: TranslateKeep;
}

/**
 * Keep-translating, as one click starts it: later pages in the same language on the
 * same site, not the one just translated (a reload is not consent), at most
 * TRANSLATE_KEEP_PAGES of them.
 */
export interface TranslateKeep {
  /** The primary language it translates from. */
  lang: string;
  /** The site the person clicked on. */
  origin: string;
  /** The last page translated, so reloading it does not translate (and bill) again. */
  url: string;
  /** Pages translated on their own since the click. */
  pages: number;
}

/** What a collaborator tells when a tab changed: the strip is sent again. */
export interface TabListSink {
  /** Sends the tab strip to the shell. */
  sendTabList(): void;
}

/** The tab list, as the strip's commands and popups change it. */
export interface TabStrip extends TabListSink {
  /** The open tabs, in strip order. */
  list: Tab[];
  /** The tab on screen, or null. */
  activeTabId: number | null;
}
