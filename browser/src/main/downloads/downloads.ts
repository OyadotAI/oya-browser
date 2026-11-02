/**
 * The person's downloads, as Chrome keeps them: a file a page the person may use
 * downloads goes straight to the Downloads folder (no save dialog), and the
 * Downloads page lists it with its progress, per profile. A page an agent drives
 * keeps Electron's Save As dialog, so a person confirms what it writes; the file is
 * listed once they pick where, under the name they chose. Agents' browser contexts have their own
 * download policy (native-downloads/) and are never installed here.
 */
import { randomUUID } from 'node:crypto';
import { basename } from 'node:path';
import type { DownloadItem, Session, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { JsonFile } from '../app/json-file.ts';
import type { DownloadEntry } from '../../shared/library.ts';
import { DOWNLOADS_KEPT, PROGRESS_INTERVAL_MS } from './constants.ts';
import { claimPath, fileExists } from './repository.ts';

/** The services downloads use: the Downloads folder and shell, and which profile is shown. */
type Deps = Pick<AppServices, 'electron' | 'persona'>;

/** A download as kept on disk: what the page shows, plus where it was saved and for which profile. */
export interface StoredDownload extends DownloadEntry {
  /** The saved file; never sent to the shell. */
  path: string;
  /** The profile's partition it was downloaded in. */
  partition: string;
}

/** The person's downloads and the sessions that report them. */
export class Downloads {
  /** Hears every change, so open Downloads pages redraw. */
  changed: () => void = () => {};
  /** Hears the page a download started from, so a tab opened only to download can close, as in Chrome. */
  started: (contents: WebContents) => void = () => {};
  /**
   * Whether the person may use the page a download started from. Only then is it saved
   * without asking: from a page an agent drives, Electron's Save As dialog stays, so a
   * person still confirms every file an agent would write to their disk.
   */
  mayAutoSave: (contents: WebContents) => boolean = () => false;
  /**
   * Hears each download as it is listed, with the page it came from, so the window that page is in can show it.
   * Told before `started`, which may close that page: a tab opened only to download is gone right after.
   */
  listedFrom: (id: string, contents: WebContents | undefined) => void = () => {};
  /** Every kept download, newest first, across profiles. */
  private entries: StoredDownload[];
  /** Sessions already reporting here. */
  private readonly installed = new WeakSet<Session>();
  /** The page each download came from, until it is listed. */
  private readonly sources = new WeakMap<DownloadItem, WebContents>();
  /** The pending progress save, so arriving bytes write at most once per interval. */
  private pending?: ReturnType<typeof setTimeout>;
  /** The Downloads folder, the shell and the profile. */
  private readonly deps: Deps;
  /** The list's file. */
  private readonly file: JsonFile<StoredDownload[]>;

  /** Reads the list; a download still running when the app quit is interrupted now. */
  constructor(deps: Deps, file: JsonFile<StoredDownload[]>) {
    this.deps = deps;
    this.file = file;
    const kept = file.read();
    this.entries = (Array.isArray(kept) ? kept : []).filter(isStoredDownload).map(settled);
  }

  /** Saves `session`'s downloads to the Downloads folder and lists them under `partition`. */
  install(session: Session, partition: string): void {
    if (this.installed.has(session)) return;
    this.installed.add(session);
    session.on('will-download', (_event, item, contents) => {
      this.sources.set(item, contents);
      if (!this.mayAutoSave(contents)) return this.awaitChoice(item, partition);
      this.start(item, partition);
      this.started(contents);
    });
  }

  /** The shown profile's downloads, newest first, without where they were saved. */
  list(): DownloadEntry[] {
    const partition = this.deps.persona.partitionName();
    return this.entries.filter((e) => e.partition === partition).map(({ path: _p, partition: _q, ...e }) => e);
  }

  /** Opens a saved file with its app; false when it is not saved or is gone. */
  open(id: string): boolean {
    const entry = this.saved(id);
    if (entry) void this.deps.electron.shell.openPath(entry.path);
    return !!entry;
  }

  /** Shows a saved file in its folder; false when it is not saved or is gone. */
  reveal(id: string): boolean {
    const entry = this.saved(id);
    if (entry) this.deps.electron.shell.showItemInFolder(entry.path);
    return !!entry;
  }

  /** Takes a download off the list (its file stays). */
  forget(id: string): void {
    this.entries = this.entries.filter((e) => e.id !== id || e.state === 'progressing');
    this.flush();
  }

  /** Clears the shown profile's finished downloads off the list (their files stay). */
  clear(): void {
    const partition = this.deps.persona.partitionName();
    this.entries = this.entries.filter((e) => e.partition !== partition || e.state === 'progressing');
    this.flush();
  }

  /** The shown profile's saved download `id`, when its file is still there. */
  private saved(id: string): StoredDownload | undefined {
    const entry = this.entries.find((e) => e.id === id && e.partition === this.deps.persona.partitionName());
    return entry?.state === 'completed' && fileExists(entry.path) ? entry : undefined;
  }

  /** A page started a download: save it in the Downloads folder under a free name, and list it. */
  private start(item: DownloadItem, partition: string): void {
    const path = claimPath(this.deps.electron.app.getPath('downloads'), item.getFilename());
    item.setSavePath(path);
    const entry = this.listed(item, path, partition, item.getFilename());
    item.on('updated', () => this.update(entry, item));
    item.once('done', (_event, state) => this.finish(entry, item, state));
  }

  /**
   * A page an agent drives started a download: Electron's Save As asks the person where, and it is listed
   * only once they pick, at the path and under the name they chose. A Save As they cancel lists nothing.
   * Electron has no "path chosen" event, but an update or the end always follows the choice.
   */
  private awaitChoice(item: DownloadItem, partition: string): void {
    let entry: StoredDownload | undefined;
    const chosen = () => (entry ??= item.getSavePath() ? this.listed(item, item.getSavePath(), partition) : undefined);
    item.on('updated', () => {
      const e = chosen();
      if (e) this.update(e, item);
    });
    // chosen() keeps the entry it made, so asking twice lists the download once.
    item.once('done', (_event, state) => chosen() && this.finish(chosen() as StoredDownload, item, state));
  }

  /** Puts a download saving at `path` at the top of the list, titled `title` (the chosen file's name by default). */
  private listed(item: DownloadItem, path: string, partition: string, title = basename(path)): StoredDownload {
    const entry: StoredDownload = { ...newDownload(item, title), id: randomUUID(), path, partition };
    this.entries = [entry, ...this.entries].slice(0, DOWNLOADS_KEPT);
    this.flush();
    this.listedFrom(entry.id, this.sources.get(item));
    return entry;
  }

  /**
   * Bytes arrived: told at most once per interval. Never changes the state: Electron reports a
   * resumable pause as `interrupted` while the download goes on, and only `done` says how it ended.
   */
  private update(entry: StoredDownload, item: DownloadItem): void {
    Object.assign(entry, bytes(item));
    this.pending ??= setTimeout(() => this.flush(), PROGRESS_INTERVAL_MS);
  }

  /** The download ended: saved, cancelled or failed, told at once. */
  private finish(entry: StoredDownload, item: DownloadItem, state: string): void {
    Object.assign(entry, bytes(item), {
      state: state === 'completed' || state === 'cancelled' ? state : 'interrupted',
    });
    this.flush();
  }

  /** Writes the list and tells the pages. */
  private flush(): void {
    clearTimeout(this.pending);
    this.pending = undefined;
    this.file.write(this.entries);
    this.changed();
  }
}

/** A new download as the page shows it: its file name `title`, where it came from, now. */
function newDownload(item: DownloadItem, title: string): DownloadEntry {
  // A data: address carries the whole file: keep only its scheme.
  const url = item.getURL().startsWith('data:') ? 'data:' : item.getURL();
  return { id: '', url, title, time: Date.now(), state: 'progressing', ...bytes(item) };
}

/** The bytes saved so far and the size. */
const bytes = (item: DownloadItem): Pick<DownloadEntry, 'received' | 'total'> => ({
  received: item.getReceivedBytes(),
  total: item.getTotalBytes(),
});

/** Whether a kept entry has what the list needs: a file read back is untrusted. */
function isStoredDownload(e: unknown): e is StoredDownload {
  const d = e as StoredDownload;
  return !!d && typeof d.id === 'string' && typeof d.path === 'string' && typeof d.partition === 'string';
}

/** A download the app quit during did not finish. */
const settled = (e: StoredDownload): StoredDownload => (e.state === 'progressing' ? { ...e, state: 'interrupted' } : e);
