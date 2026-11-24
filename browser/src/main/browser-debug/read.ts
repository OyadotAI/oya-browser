/** Frame-labelled reads reserve headings, footers and UTF-8 bytes before publishing element numbers. */
import type { WebFrameMain } from 'electron';
import type { DebugDocument, DebugRegistry } from './registry.ts';
import { debugFrames, linkedDocument } from './frames.ts';
import { BROWSER_DEBUG as C } from '../../shared/browser-debug.ts';
/** Local nodes exclude descendant documents and redact password field values. */
interface ReadData {
  /** Document title retained by default and framed reads. */ title?: string;
  /** Visible local text. */ text: string;
  /** Exact local controls. */ elements: ReadRow[];
}
/** One isolated read control. */
interface ReadRow {
  /** Private node id. */ id: number;
  /** Semantic role. */ role: string;
  /** Accessible label. */ name: string;
  /** Link destination. */ href?: string | null;
  /** Bounded form state and available options. */ facts?: Record<string, unknown>;
  /** Actual interception. */ covered: boolean;
}
/** Failed frames still occupy an explicit labelled section. */
interface Section {
  /** Contacted document. */ doc?: DebugDocument;
  /** Native URL. */ url: string;
  /** Local read. */ read?: ReadData;
  /** Failure. */ error?: string;
}
const READ_MS = 12000,
  TEXT_BYTES = 60000,
  URL_BYTES = 256,
  SECTION_RESERVE = 192,
  SUMMARY_RESERVE = 128;
/** Inspect independently registered descendants, including duplicate-origin sibling documents. */
export async function readDebugFrames(registry: DebugRegistry, all = true) {
  registry.beginRead();
  const frames = debugFrames(registry).filter((f) => all || f === registry.page.webContents.mainFrame);
  const sections: Section[] = [],
    deadline = Date.now() + READ_MS;
  for (const frame of frames) sections.push(await inspectDebugSection(registry, frame, deadline));
  return formatDebugRead(registry, sections);
}
/** A frame failure does not erase the readable sections or imply complete coverage. */
async function inspectDebugSection(registry: DebugRegistry, frame: WebFrameMain, deadline: number): Promise<Section> {
  if (Date.now() >= deadline) return { url: frame.url, error: 'Inspection deadline reached' };
  try {
    const doc = await linkedDocument(registry, frame);
    return { doc, url: doc.url, read: await registry.run<ReadData>(doc, 'read') };
  } catch (why) {
    return { url: frame.url, error: String((why as Error)?.message ?? why) };
  }
}
/** Reserve all labels and fixed footers before distributing body space. */
function formatDebugRead(registry: DebugRegistry, sections: Section[]) {
  const headings = sections.map(debugSectionHeading),
    read = sections.filter((s) => s.read).length,
    unread = sections.length - read;
  const remaining =
    TEXT_BYTES - Buffer.byteLength(headings.join('')) - SUMMARY_RESERVE - sections.length * SECTION_RESERVE;
  const budget = Math.max(0, Math.floor(remaining / Math.max(1, sections.length)));
  const text = sections.map((s, i) => headings[i] + debugSectionBody(registry, s, budget)).join('\n\n');
  return {
    text: `${text}\n\nFrames: ${read} read; ${unread} unread.`,
    data: { frames: sections.map(debugSectionInventory), read, unread },
  };
}
/** URLs are visibly shortened for output only and never used as authority. */
function debugSectionHeading(s: Section): string {
  return `## Frame ${s.doc?.ref ?? 'unread'} · parent ${s.doc?.owner?.parent.ref ?? 'none'}\nURL: ${debugBytePrefix(s.url, URL_BYTES)}\nTitle: ${debugBytePrefix(s.read?.title ?? '', C.MAX_LABEL)}\n`;
}
/** Structured inventory carries the same coverage as the human-readable sections. */
function debugSectionInventory(s: Section) {
  return {
    ref: s.doc?.ref ?? null,
    parent: s.doc?.owner?.parent.ref ?? null,
    url: debugBytePrefix(s.url, URL_BYTES),
    readable: !!s.read,
  };
}
/** Only element numbers actually printed here become capabilities. */
function debugSectionBody(registry: DebugRegistry, s: Section, budget: number): string {
  if (!s.read || !s.doc) return `Unread: ${debugBytePrefix(s.error ?? 'No response', budget + SECTION_RESERVE)}`;
  const lines = debugElementLines(registry, s.doc, s.read.elements, budget);
  const available = Math.max(0, budget - Buffer.byteLength(lines.join('\n')));
  return `${debugBytePrefix(s.read.text, available)}\n### Elements\n${lines.join('\n')}\n${s.read.elements.length - lines.length} elements omitted by the result limit.`;
}
/** Budget includes the largest supported element number and JSON-escaped labels. */
function debugElementLines(registry: DebugRegistry, doc: DebugDocument, rows: ReadRow[], budget: number): string[] {
  const lines: string[] = [];
  let used = 0;
  for (const row of rows) {
    const line = `${row.role} ${JSON.stringify(row.name)}${row.href ? ` → ${debugBytePrefix(row.href, URL_BYTES)}` : ''}${row.covered ? ' (covered)' : ''}${row.facts && Object.keys(row.facts).length ? ' ' + JSON.stringify(row.facts) : ''}`;
    const size = Buffer.byteLength(line) + C.LINE_RESERVE;
    if (used + size > budget) break;
    lines.push(`[${registry.element(doc, row.id)}] ${line}`);
    used += size;
  }
  return lines;
}
/** Shortening preserves UTF-8 characters and includes its notice inside the byte limit. */
export function debugBytePrefix(text: string, limit: number): string {
  const bytes = Buffer.from(text),
    notice = '… [shortened]';
  if (bytes.length <= limit) return text;
  let end = Math.max(0, limit - Buffer.byteLength(notice));
  while (end > 0 && (bytes[end] & C.UTF8_MASK) === C.UTF8_CONTINUATION) end--;
  return bytes.subarray(0, end).toString() + (limit >= Buffer.byteLength(notice) ? notice : '');
}
