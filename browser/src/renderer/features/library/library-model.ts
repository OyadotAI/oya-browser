/** Literal search, site filters and calendar groups for the private library view. */
import type { SavedPage } from '../../../shared/library.ts';
/** Select controls use fixed sort choices. */
export type LibrarySort = 'recent' | 'title' | 'site';
/** Every whitespace-separated word must occur somewhere in the page title or address. */
export function matching(pages: SavedPage[], query: string, site: string, sort: LibrarySort): SavedPage[] {
  const terms = query.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
  const found = pages.filter(
    (page) =>
      (!site || host(page) === site) &&
      terms.every((term) => `${page.title} ${page.url}`.toLocaleLowerCase().includes(term)),
  );
  return found.sort((a, b) => compare(a, b, sort));
}
/** Full host disambiguates similarly named saved pages, without tracking favicon requests. */
export function host(page: SavedPage): string {
  try {
    return new URL(page.url).host;
  } catch {
    return '';
  }
}
/** Local calendar boundaries keep yesterday correct across daylight-saving changes. */
export function dayLabel(time: number, now = new Date()): string {
  const date = new Date(time);
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (date.toDateString() === now.toDateString()) return 'Today';
  if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
  return date.toLocaleDateString(undefined, { weekday: 'long', month: 'short', day: 'numeric', year: 'numeric' });
}
/** Bytes in the next unit up. */
const BYTES_PER_UNIT = 1024;
/** Size units, smallest first. */
const SIZE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
/** Sizes of ten units and more need no decimal. */
const WHOLE_FROM = 10;

/** A byte count as people read it: "512 B", "3.4 MB", "12 GB". */
export function fileSize(bytes: number): string {
  let [value, unit] = [Math.max(bytes, 0), 0];
  while (value >= BYTES_PER_UNIT && unit < SIZE_UNITS.length - 1) [value, unit] = [value / BYTES_PER_UNIT, unit + 1];
  const shown = unit === 0 || value >= WHOLE_FROM ? Math.round(value) : value.toFixed(1);
  return `${shown} ${SIZE_UNITS[unit]}`;
}
/** Most-used sites come first, with stable alphabetical ties; hostless URLs stay under All websites. */
export function sites(pages: SavedPage[]): [string, number][] {
  const counts = new Map<string, number>();
  for (const page of pages) {
    const name = host(page);
    if (name) counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

/** Compare without mutating the source snapshot's order. */
function compare(a: SavedPage, b: SavedPage, sort: LibrarySort): number {
  if (sort === 'title') return a.title.localeCompare(b.title);
  if (sort === 'site') return host(a).localeCompare(host(b)) || b.time - a.time;
  return b.time - a.time;
}
