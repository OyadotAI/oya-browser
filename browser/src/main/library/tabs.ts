/** Private library tabs use shell content; their ordinary blank web views never contain browsing data. */
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { LibraryMenu } from './menu.ts';
import type { LibraryKind, LibraryAction, LibraryPresentation } from '../../shared/library.ts';
import type { DownloadAction } from '../../shared/ipc.ts';
/** Window-scoped services keep a collection attached to its owning tab after transfers. */
/** Full services are needed only for native maintenance menus. */
type MenuDeps = ConstructorParameters<typeof LibraryMenu>[0];
/** Read and saved-page operations need no native window services. */
type Deps = Pick<AppServices, 'tabs' | 'library' | 'downloads' | 'persona' | 'agentTab'>;
/** Open once per collection per window, then use normal selection, close and transfer behavior. */
export function showLibrary(deps: Deps, kind: LibraryKind): void {
  deps.agentTab.requireFor(null);
  const existing = deps.tabs.list.find((tab) => tab.collection === kind);
  if (existing) return deps.tabs.activateTab(existing.id);
  const id = deps.tabs.createTab(HOME_URL, false);
  const tab = deps.tabs.find(id)!;
  describeTab(deps, tab, kind);
  deps.tabs.activateTab(id);
}
/** Exact-shell IPC additionally checks active tab, profile and ownership before returning private data. */
export function readLibrary(deps: Deps, id: number): LibraryPresentation {
  const tab = deps.tabs.find(id);
  deps.agentTab.requireFor(id);
  if (!tab?.collection || id !== deps.tabs.activeTabId || tab.libraryPartition !== deps.persona.partitionName())
    throw Error('This library is no longer available');
  return { ...deps.library.snapshot(), downloads: deps.downloads.list(), kind: tab.collection };
}
/** Fixed renderer command arguments after the shell sender has been checked. */
type LibraryCommand = [id: number, action: LibraryAction, url?: string];
/** Actions can only name an existing library page, never an arbitrary renderer-supplied URL. */
export function changeLibrary(deps: MenuDeps, ...args: LibraryCommand): LibraryPresentation | void {
  const [id, action, url] = args;
  const snapshot = readLibrary(deps, id);
  if (action === 'manage') return new LibraryMenu(deps).manage(snapshot.kind);
  changeEntry(deps, snapshot, action, url);
  return action === 'open' || action === 'reveal' ? undefined : readLibrary(deps, id);
}

/** Each collection's tab title. */
const TITLES: Record<LibraryKind, string> = { bookmarks: 'Bookmarks', history: 'History', downloads: 'Downloads' };

/** Collection identity is native metadata, not a URL any website can impersonate. */
function describeTab(deps: Deps, tab: Tab, kind: LibraryKind): void {
  Object.assign(tab, {
    collection: kind,
    libraryPartition: deps.persona.partitionName(),
    title: TITLES[kind],
    url: `oya://${kind}`,
  });
}

/** What a download's row can do, by id: open the file, show it in its folder, take it off the list. */
const DOWNLOAD_ACTIONS: Partial<Record<LibraryAction, (deps: Deps, id: string) => unknown>> = {
  open: (deps, id) => deps.downloads.open(id) || fileGone(),
  reveal: (deps, id) => deps.downloads.reveal(id) || fileGone(),
  remove: (deps, id) => deps.downloads.forget(id),
};

/** Opens or reveals download `id` for the toolbar's chip; the file's absence is thrown for it to say. */
export function runDownloadAction(deps: Deps, id: string, action: DownloadAction): void {
  const run = Object.hasOwn(CHIP_ACTIONS, action) ? CHIP_ACTIONS[action] : undefined;
  if (!run) throw Error('Unknown download action');
  run(deps, id);
}

/** What the toolbar's chip can do with a saved download: the row's open and reveal, never remove. */
const CHIP_ACTIONS: Record<DownloadAction, (deps: Deps, id: string) => unknown> = {
  open: DOWNLOAD_ACTIONS.open!,
  reveal: DOWNLOAD_ACTIONS.reveal!,
};

/** Throws that a download's file was moved or deleted, for the page to say. */
function fileGone(): never {
  throw Error('This file was moved or deleted');
}

/** Validate and apply a single idempotent saved-page action; a download's names its id, not an address. */
function changeEntry(deps: Deps, snapshot: LibraryPresentation, action: LibraryAction, url?: string): void {
  if (snapshot.kind === 'downloads') return changeDownload(deps, snapshot, action, url);
  const entry = [...snapshot.bookmarks, ...snapshot.history].find((page) => page.url === url);
  if (!entry) throw Error('This page is no longer in your library');
  if (action === 'bookmark') deps.library.addBookmark(entry.url, entry.title);
  else if (action === 'remove') deps.library.removeBookmark(entry.url);
  else if (action === 'open') return void deps.tabs.createTab(entry.url, true);
  else throw Error('Unknown library action');
}

/** Runs a download row's action, for a download this profile lists. */
function changeDownload(deps: Deps, snapshot: LibraryPresentation, action: LibraryAction, id?: string): void {
  const run = Object.hasOwn(DOWNLOAD_ACTIONS, action) ? DOWNLOAD_ACTIONS[action] : undefined;
  if (!run || !snapshot.downloads.some((d) => d.id === id)) throw Error('This download is no longer in your list');
  run(deps, id!);
}
