/** Read the MCP URL and credential, never mistaking a named stale entry for this browser. */
import type { Endpoint } from './agents.ts';

/** The tiny public shape used only within main for comparison. */
interface Entry {
  /** A literal HTTP endpoint. */ url?: unknown;
  /** Authorization remains private and is never returned through IPC. */ headers?: Record<string, unknown>;
}
/** JSON editors and Claude Code store the same endpoint and authorization fields. */
export function matchesEndpoint(value: unknown, endpoint: Endpoint): boolean {
  if (!value || typeof value !== 'object') return false;
  const entry = value as Entry;
  return entry.url === endpoint.url && entry.headers?.Authorization === `Bearer ${endpoint.token}`;
}
/** Literal strings written by Oya and native Codex configs are accepted; unsupported syntax fails closed. */
function tomlString(raw: string): string | null {
  const match = raw.trim().match(/^("(?:[^"\\]|\\.)*"|'[^']*')\s*(?:#.*)?$/);
  if (!match) return null;
  try {
    return match[1].startsWith('"') ? JSON.parse(match[1]) : match[1].slice(1, -1);
  } catch {
    return null;
  }
}
/** A table is a sequence of components; dots within quotes never split a component. */
const TABLE_PATH = /^(?:"(?:[^"\\]|\\.)*"|'[^']*'|[\w-]+)(?:\s*\.\s*(?:"(?:[^"\\]|\\.)*"|'[^']*'|[\w-]+))*$/;
/** Canonical component boundaries distinguish a root key containing dots from nested tables. */
const SERVER_TABLE = JSON.stringify(['mcp_servers', 'oya-browser']);
/** Native header subtables use exactly one additional logical component. */
const HEADERS_TABLE = JSON.stringify(['mcp_servers', 'oya-browser', 'http_headers']);
/** Parse a table's components instead of searching inside comments or string values. */
function sectionOf(line: string): string | null {
  if (!line.trimStart().startsWith('[')) return null;
  const match = line.match(/^\s*\[([^\]]+)\]\s*(?:#.*)?$/);
  if (!match || !TABLE_PATH.test(match[1].trim())) return '';
  const tokens = match[1].match(/"(?:[^"\\]|\\.)*"|'[^']*'|[\w-]+/g) || [];
  return JSON.stringify(tokens.map((s) => tomlString(s) ?? s));
}
/** Only a literal Authorization mapping is accepted; no environment substitution is inferred. */
function inlineAuth(raw: string): string | null {
  const match = raw.match(
    /^\s*\{\s*(?:Authorization|"Authorization"|'Authorization')\s*=\s*(.*?)\s*,?\s*\}\s*(?:#.*)?$/,
  );
  return match ? tomlString(match[1]) : null;
}
/** A parsed field has no implied default for an unreadable value. */
interface Fields {
  /** Parsed URL or invalid literal. */ url?: string | null;
  /** Parsed Authorization or invalid literal. */ auth?: string | null;
}
/** Ignore unrelated tables; only this server's own fields establish a match. */
function readField(section: string, key: string, raw: string): Fields | null {
  if (section === SERVER_TABLE && key === 'url') return { url: tomlString(raw) };
  if (section === SERVER_TABLE && key === 'http_headers') return { auth: inlineAuth(raw) };
  if (section === HEADERS_TABLE && key.replace(/["']/g, '') === 'Authorization') return { auth: tomlString(raw) };
  return null;
}
/** A fail-closed structured reader tracks table and field ownership and rejects duplicates. */
class CodexReader {
  /** Current logical table. */ private section = '';
  /** Duplicate fields never choose an arbitrary last value. */ private readonly seen = new Set<string>();
  /** Values remain inside main. */ private readonly values: Fields = {};
  /** Unsupported or ambiguous syntax cannot establish a green status. */ private invalid = false;
  /** Multiline strings are refused rather than parsing fake headers inside them. */
  read(text: string): Entry | null {
    if (/"""|'''/.test(text)) return null;
    for (const line of text.split(/\r?\n/)) this.line(line);
    return !this.invalid && this.values.url && this.values.auth
      ? { url: this.values.url, headers: { Authorization: this.values.auth } }
      : null;
  }
  /** A line belongs to the current section, never a comment's matching substring. */
  private line(line: string): void {
    const next = sectionOf(line);
    if (next !== null) {
      this.section = next;
      return;
    }
    const field = line.match(/^\s*([\w-]+|"Authorization"|'Authorization')\s*=\s*(.*)$/);
    if (field) this.record(readField(this.section, field[1], field[2]));
  }
  /** Only fields relevant to Oya participate in duplicate checks. */
  private record(fields: Fields | null): void {
    for (const [name, value] of Object.entries(fields || {})) this.field(name as keyof Fields, value);
  }
  /** Malformed literals and repeated URL/credential values fail closed. */
  private field(name: keyof Fields, value: string | null): void {
    if (this.seen.has(name) || value === null) this.invalid = true;
    this.seen.add(name);
    this.values[name] = value;
  }
}
/** Read a logical server record, including native header subtables and quoted table components. */
export const codexEndpoint = (text: string): Entry | null => new CodexReader().read(text);
