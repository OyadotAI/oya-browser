/**
 * What the toolbar's download chip says, worked out from what main sends: the
 * phase of the window's latest download, its words for the screen, a screen
 * reader and the tooltip, and the ring for the downloads still running. It
 * says "Saved" only for a download main reports completed, and "Failed" only
 * once one has ended: main never reports a pause as a failure.
 */
import type { DownloadBubbleState } from '../../../../shared/ipc.ts';
import type { DownloadEntry } from '../../../../shared/library.ts';
import { fileSize } from '../../library/library-model.ts';
import { DOWNLOAD_TEXT, PERCENT } from './constants.ts';

/** Where the chip stands: the plain Downloads button, or the latest download's state. */
export type ChipPhase = 'idle' | 'running' | 'saved' | 'failed' | 'cancelled' | 'gone';

/** The phases a download has ended in, which close on their own. */
export const ENDED: ReadonlySet<ChipPhase> = new Set(['saved', 'failed', 'cancelled', 'gone']);

/** What the chip draws. */
export interface DownloadChipView {
  /** Its phase. */
  phase: ChipPhase;
  /** The download it shows; '' when idle. */
  id: string;
  /** The status word ("Saved", "Downloading 2 files"). */
  status: string;
  /** The file's name up to its extension, which may be cut short. */
  stem: string;
  /** Its extension, never cut ("" when it has none). */
  ext: string;
  /** Whether the name shows: not while several run and the chip counts them. */
  named: boolean;
  /** The body's accessible name. */
  label: string;
  /** The body's tooltip. */
  title: string;
  /** The ring and the bar: the share saved of what is running, null when a size is unknown, false when nothing runs. */
  ring: number | null | false;
  /** How far the running downloads are, in words: "1.2 MB of 4.5 MB · 27%", or "1.2 MB so far"; '' when none runs. */
  meter: string;
}

/** The plain Downloads button. */
const IDLE: DownloadChipView = {
  ...{ phase: 'idle', id: '', status: '', stem: '', ext: '', named: false },
  ...{ label: DOWNLOAD_TEXT.button, title: DOWNLOAD_TEXT.button + DOWNLOAD_TEXT.keys, ring: false, meter: '' },
};

/** A download's phase: `gone` once Open or Show found its file missing. */
export function phaseOf(entry: DownloadEntry, gone: boolean): ChipPhase {
  if (gone) return 'gone';
  if (entry.state === 'progressing') return 'running';
  if (entry.state === 'completed') return 'saved';
  return entry.state === 'cancelled' ? 'cancelled' : 'failed';
}

/** A file name split so its extension is never cut: "report.final.pdf" → ["report.final", ".pdf"]. */
export function splitName(name: string): [string, string] {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, ''];
}

/** The ring for what is still running. */
function ringOf(s: DownloadBubbleState): number | null | false {
  if (!s.running) return false;
  return s.total ? Math.min(s.received / s.total, 1) : null;
}

/** How far everything running is: bytes of the total and the percent, or bytes so far when a size is unknown. */
export function meterOf(s: DownloadBubbleState): string {
  if (!s.running) return '';
  const received = fileSize(s.received);
  if (!s.total) return DOWNLOAD_TEXT.soFar(received);
  const percent = Math.floor((Math.min(s.received, s.total) / s.total) * PERCENT);
  return DOWNLOAD_TEXT.meter(received, fileSize(s.total), percent);
}

/** The body's description, before what clicking it does. */
function describe(entry: DownloadEntry, phase: ChipPhase, s: DownloadBubbleState): string {
  const say = DOWNLOAD_TEXT.describe;
  const name = entry.title;
  if (phase === 'running' && s.running > 1) return say.files(s.running);
  if (phase === 'running') return say.running(name, bytesOf(entry));
  if (phase === 'saved') return say.saved(name, fileSize(entry.total || entry.received));
  return say[phase as 'failed' | 'cancelled' | 'gone'](name);
}

/** A running download's bytes: "12 KB of 57 KB", or "12 KB so far" when the site gave no size. */
function bytesOf(entry: DownloadEntry): string {
  const received = fileSize(entry.received);
  return entry.total ? DOWNLOAD_TEXT.of(received, fileSize(entry.total)) : DOWNLOAD_TEXT.soFar(received);
}

/** The status word for `phase`, or the count when several run and the latest is one of them. */
function statusOf(phase: ChipPhase, s: DownloadBubbleState): string {
  if (phase === 'running' && s.running > 1) return DOWNLOAD_TEXT.files(s.running);
  return DOWNLOAD_TEXT[phase as Exclude<ChipPhase, 'idle'>];
}

/** What the chip draws for main's `s` in `phase` (idle when there is nothing to show). */
export function chipView(s: DownloadBubbleState | null, phase: ChipPhase): DownloadChipView {
  const entry = s?.latest;
  if (!s || !entry || phase === 'idle') return { ...IDLE, ring: s ? ringOf(s) : false, meter: s ? meterOf(s) : '' };
  const [stem, ext] = splitName(entry.title);
  const text = describe(entry, phase, s);
  const status = statusOf(phase, s);
  const named = !(phase === 'running' && s.running > 1);
  const words = { status, stem, ext, named, label: text + DOWNLOAD_TEXT.showAll, title: text + DOWNLOAD_TEXT.keys };
  return { phase, id: entry.id, ...words, ring: ringOf(s), meter: meterOf(s) };
}

/** What a screen reader hears when the latest download reaches `phase`; '' for nothing. */
export function announcement(phase: ChipPhase, name: string): string {
  if (phase === 'idle') return '';
  return DOWNLOAD_TEXT.say[phase](name);
}
