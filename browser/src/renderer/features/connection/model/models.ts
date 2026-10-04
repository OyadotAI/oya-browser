/**
 * The shapes this feature reads from the main process, where the shared IPC
 * contract only says `Payload` (src/shared/ipc.ts), and the pure helpers its
 * ViewModels share: reading a payload, and an error's words.
 */

/** When the logins last reached the server, as config.json keeps it. */
export interface LastSync {
  /** When, in epoch milliseconds. */
  at: number;
  /** How many sites the server said it keeps. */
  sites: number;
}

/** One finished login import, as the main process keeps it (config `imports`). */
export interface ImportRecord {
  /** The browser's name ("Chrome"). */
  source: string;
  /** When it finished, in epoch milliseconds. */
  at: number;
  /** Sites it brought, when it counted them. */
  sites?: number;
  /** Cookies it brought. */
  cookies?: number;
  /** Browser profiles it read. */
  profiles?: number;
}

/** The saved settings (src/main/app/config-store.ts), the fields this feature reads. */
export interface Config {
  /** The server's address. */
  serverUrl?: string;
  /** The API key, when one is saved. */
  apiKey?: string;
  /** This browser's name in the dashboard. */
  browserName?: string;
  /** The last Sync now the server confirmed. */
  lastSync?: LastSync;
  /** The last few login imports, newest first. */
  imports?: ImportRecord[];
}

/** Who this browser is signed in as (get-account). */
export interface Account {
  /** The person's name. */
  name?: string | null;
  /** The person's email. */
  email?: string | null;
  /** The plan, when the server has plans. */
  plan?: string | null;
  /** The project this browser belongs to. */
  project?: Project | null;
}

/** A project on the server. */
export interface Project {
  /** Its id. */
  id?: string;
  /** Its name. */
  name?: string;
}

/** A browser on this computer that logins can be imported from. */
export interface ImportSource {
  /** Its id for reimportBrowser. */
  id: string;
  /** Its name ("Firefox"). */
  name: string;
  /** Whether it is the person's default browser. */
  isDefault?: boolean;
}

/** A login import's progress (mirror-status). */
export interface MirrorStatus extends Partial<ImportRecord> {
  /** The import started. */
  started?: boolean;
  /** Why it failed. */
  error?: string;
  /** The browser had nothing to bring. */
  empty?: boolean;
}

/** The persona's device, as sites see it. */
export interface Device {
  /** navigator.platform ("Win32"). */
  platform?: string;
  /** The time zone. */
  timezone?: string;
  /** The locale. */
  locale?: string;
  /** The screen size ("1920x1080"). */
  screen?: string;
}

/** The server's answer to Sync now (profile-saved). */
export interface ProfileSaved {
  /** The sites the server keeps. */
  sites?: string[];
  /** Why it failed. */
  error?: string;
}

/** The updater's state (src/main/app/updater.ts). */
export interface UpdateStatus {
  /** checking, available, downloading, ready, error, unsupported, or anything meaning up to date. */
  state?: string;
  /** The running version. */
  current?: string;
  /** The version on offer. */
  version?: string;
  /** How far the download is. */
  percent?: number;
}

/** A payload read as `T`: the main process owns these shapes, so this is the one place they are trusted. */
export function shape<T extends object>(value: unknown): T {
  return (value && typeof value === 'object' ? value : {}) as T;
}

/** Like `shape`, keeping "nothing" as null. */
export function shapeOrNull<T extends object>(value: unknown): T | null {
  return value && typeof value === 'object' ? (value as T) : null;
}

/** An error's words for the person, or `fallback` when it has none. */
export function messageOf(error: unknown, fallback = ''): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

/** What a view of one ViewModel is given. */
export interface ViewProps<V> {
  /** The ViewModel it reads and calls. */
  vm: V;
}

/** What a page of the shell dialog is given. */
export interface PageProps<V> extends ViewProps<V> {
  /** Another page is in view. */
  hidden: boolean;
}
