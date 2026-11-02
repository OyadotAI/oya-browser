/** Accessible, literal DOM for local library rows; no remote icons or unsafe markup. */
import { host, fileSize } from './library-model.ts';
import type { SavedPage, LibraryAction, DownloadEntry } from '../../../shared/library.ts';
import { isMac } from '../../../shared/shortcuts.ts';
/** Website titles remain plain text even when they contain markup. */
export function text(tag: string, className: string, value: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  node.textContent = value;
  return node;
}
/** Native buttons give every row and action keyboard and assistive-technology support. */
export function button(label: string, className: string, run: () => void): HTMLButtonElement {
  const node = document.createElement('button');
  node.className = className;
  node.textContent = label;
  node.addEventListener('click', run);
  return node;
}
/** The title and readable URL take the space, with a local site monogram (or a download's file badge) for recognition. */
function pageLink(page: SavedPage, open: () => void, mark = monogram(page)): HTMLElement {
  const link = button('', 'page-link', open);
  const info = text('span', 'page-info', '');
  info.append(text('strong', 'page-title', page.title || host(page)), text('span', 'page-url', page.url));
  link.append(mark, info);
  link.title = `Open in new tab: ${page.url}`;
  return link;
}
/** A saved-state action stays visible and names exactly which page it changes. */
export function row(page: SavedPage, saved: boolean, action: (kind: LibraryAction, url: string) => void): HTMLElement {
  const item = text('li', 'page-row', '');
  item.dataset.url = page.url;
  item.append(
    pageLink(page, () => action('open', page.url)),
    visitTime(page),
    saveButton(page, saved, action),
  );
  return item;
}

/** Site recognition without sending any history to a favicon service. */
function monogram(page: SavedPage): HTMLElement {
  return text(
    'span',
    'site-icon',
    host(page)
      .replace(/^www\./, '')
      .charAt(0)
      .toUpperCase(),
  );
}
/** Visible time also exposes the complete local date on hover. */
function visitTime(page: SavedPage): HTMLElement {
  const node = text(
    'time',
    'page-time',
    new Date(page.time).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }),
  );
  node.title = new Date(page.time).toLocaleString();
  return node;
}
/** A labeled toggle remains keyboard accessible after its row refreshes. */
function saveButton(page: SavedPage, saved: boolean, action: (kind: LibraryAction, url: string) => void): HTMLElement {
  const save = button(saved ? '★' : '☆', 'save-page', () => action(saved ? 'remove' : 'bookmark', page.url));
  save.setAttribute('aria-label', `${saved ? 'Remove bookmark' : 'Bookmark'}: ${page.title}`);
  save.setAttribute('aria-pressed', String(saved));
  save.title = saved ? 'Remove bookmark' : 'Save bookmark';
  return save;
}

/** What a download's row says about it: progress, its size, or why it stopped. */
function downloadStatus(d: DownloadEntry): string {
  if (d.state === 'cancelled') return 'Cancelled';
  if (d.state === 'interrupted') return 'Failed';
  if (d.state === 'completed') return fileSize(d.total || d.received);
  return d.total ? `${fileSize(d.received)} of ${fileSize(d.total)}` : `${fileSize(d.received)}…`;
}

/** The reveal action's words, as the platform names its file browser (the toolbar's chip says the same). */
const SHOW_FILE = isMac(globalThis.navigator?.platform ?? '') ? 'Show in Finder' : 'Show in folder';

/** A download's actions: show a saved file in its folder, and take a finished one off the list. */
function downloadButtons(d: DownloadEntry, action: (kind: LibraryAction, id: string) => void): HTMLElement {
  const group = text('span', 'download-actions', '');
  if (d.state === 'completed') group.append(button(SHOW_FILE, 'download-reveal', () => action('reveal', d.id)));
  if (d.state !== 'progressing') group.append(removeButton(d, action));
  return group;
}

/** Takes a finished download off the list; its file stays. */
function removeButton(d: DownloadEntry, action: (kind: LibraryAction, id: string) => void): HTMLElement {
  const remove = button('×', 'download-remove', () => action('remove', d.id));
  remove.setAttribute('aria-label', `Remove from list: ${d.title}`);
  remove.title = 'Remove from list (the file stays)';
  return remove;
}

/** One download: its file name (opens it once saved), where it came from, its status and actions. */
export function downloadRow(d: DownloadEntry, action: (kind: LibraryAction, id: string) => void): HTMLElement {
  const item = text('li', `page-row download-row ${d.state}`, '');
  item.dataset.url = d.id;
  const link = pageLink(d, () => action('open', d.id), fileBadge(d.title)) as HTMLButtonElement;
  link.disabled = d.state !== 'completed';
  link.title = d.state === 'completed' ? `Open ${d.title}` : d.title;
  item.append(link, text('span', 'download-status', downloadStatus(d)), downloadButtons(d, action));
  return item;
}

/** The longest extension a file badge shows. */
const BADGE_EXTENSION_MAX = 4;

/** A file's badge: its extension ("PDF", "ZIP"), or "FILE" when it has none, in the monogram's square. */
function fileBadge(name: string): HTMLElement {
  const dot = name.lastIndexOf('.');
  const ext = dot > 0 ? name.slice(dot + 1, dot + 1 + BADGE_EXTENSION_MAX).toUpperCase() : 'FILE';
  return text('span', 'site-icon file-badge', ext);
}
