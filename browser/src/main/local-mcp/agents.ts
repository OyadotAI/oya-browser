/**
 * The coding agents on this computer that can use Oya's MCP port: which are
 * installed, which already list Oya, and how to add or remove Oya from each.
 * Oya only ever touches its own entry ("oya-browser"), and only when the person
 * clicks Connect or Disconnect. An agent's own CLI makes the change where it has
 * one, so its config file is never written underneath it.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { agentCommand, runAgent, vscodeConfig } from './agent-platform.ts';
import { MCP_SERVER_NAME } from './constants.ts';
import { PRIVATE_FILE_MODE } from '../app/constants.ts';
import { matchesEndpoint, codexEndpoint } from './config-match.ts';

/** Spaces JSON configs are indented with, as the agents write them. */
const JSON_INDENT = 2;

/** Where Oya's port is and the token it wants. */
export interface Endpoint {
  /** The MCP URL. */
  url: string;
  /** The bearer token. */
  token: string;
}

/** One agent Oya can be added to. */
interface AgentTarget {
  /** Its name, as the footer shows it. */
  name: string;
  /** Whether it is on this computer. */
  installed(): boolean;
  /** Whether its config lists Oya. */
  connected(endpoint?: Endpoint): boolean;
  /** Adds Oya to its config. */
  connect(endpoint: Endpoint): Promise<void>;
  /** Removes Oya from its config. */
  disconnect(): Promise<void>;
}

/** An agent as the footer lists it. */
export interface AgentStatus {
  /** Its id. */
  id: string;
  /** Its name. */
  name: string;
  /** Whether its config lists Oya. */
  connected: boolean;
}

/** A path under the home folder. */
const home = (...parts: string[]) => path.join(os.homedir(), ...parts);

/** A JSON file's contents, {} when absent; a file that is not plain JSON is refused rather than overwritten. */
function readJson(file: string): Record<string, Record<string, unknown>> {
  if (!fs.existsSync(file)) return {};
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    throw new Error(`${file} has comments or is not plain JSON; add Oya there by hand.`);
  }
}

/** Sets (or, with null, removes) Oya's entry under `key` in JSON config `file`. */
function writeJsonEntry(file: string, key: string, entry: object | null): void {
  const config = readJson(file);
  const servers = { ...(config[key] ?? {}) };
  if (entry) servers[MCP_SERVER_NAME] = entry;
  else delete servers[MCP_SERVER_NAME];
  writePrivate(file, JSON.stringify({ ...config, [key]: servers }, null, JSON_INDENT) + '\n');
}

/** Writes (or with `append`, adds to) a config that now holds Oya's token, readable by this user only. */
function writePrivate(file: string, text: string, append = false): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  (append ? fs.appendFileSync : fs.writeFileSync)(file, text, { mode: PRIVATE_FILE_MODE });
  fs.chmodSync(file, PRIVATE_FILE_MODE); // a file that existed before keeps its old mode otherwise
}

/** An agent whose MCP servers sit in JSON file `file()` (resolved when used) under `key`, each entry shaped by `entry`. */
function jsonAgent(name: string, file: () => string, key: string, entry: (e: Endpoint) => object): AgentTarget {
  return {
    name,
    installed: () => fs.existsSync(path.dirname(file())),
    connected: (endpoint) => jsonConfigured(file(), key, endpoint),
    connect: async (e) => writeJsonEntry(file(), key, entry(e)),
    disconnect: async () => writeJsonEntry(file(), key, null),
  };
}

/** Named legacy entries and instance-matched entries share the same safe JSON read. */
function jsonConfigured(file: string, key: string, endpoint?: Endpoint): boolean {
  const servers = readJsonQuietly(file)[key] ?? {};
  return endpoint ? matchesEndpoint(servers[MCP_SERVER_NAME], endpoint) : Object.hasOwn(servers, MCP_SERVER_NAME);
}
/** A JSON file's contents, {} when it cannot be read. */
function readJsonQuietly(file: string): Record<string, Record<string, unknown>> {
  try {
    return readJson(file);
  } catch {
    return {};
  }
}

/** The header every entry sends. */
const auth = ({ token }: Endpoint) => ({ Authorization: `Bearer ${token}` });

/** Claude Code's `mcp add` for Oya's port, at user scope, sending the token. */
const claudeAdd = ({ url, token }: Endpoint) => [
  ...['mcp', 'add', '--scope', 'user', '--transport', 'http', MCP_SERVER_NAME, url],
  ...['--header', `Authorization: Bearer ${token}`],
];

/** Claude Code's permission rule that lets every Oya tool run without a prompt. */
const CLAUDE_RULE = `mcp__${MCP_SERVER_NAME}`;

/** Adds (or with `allow` false, removes) Oya's rule in Claude Code's user settings, leaving every other rule as it was. */
export function allowInClaude(allow: boolean): void {
  const file = home('.claude', 'settings.json');
  const settings = readJson(file) as Record<string, Record<string, unknown>>;
  const permissions = { ...(settings.permissions ?? {}) };
  const rules = (Array.isArray(permissions.allow) ? permissions.allow : []).filter((r) => r !== CLAUDE_RULE);
  permissions.allow = allow ? [...rules, CLAUDE_RULE] : rules;
  writePrivate(file, JSON.stringify({ ...settings, permissions }, null, JSON_INDENT) + '\n');
}

/** Claude Code: its own CLI adds and removes the server, at user scope so every project sees it; Oya's tools are allowed with it. */
const CLAUDE_CODE: AgentTarget = {
  name: 'Claude Code',
  installed: () => agentCommand('claude') !== null,
  connected: (endpoint) =>
    endpoint
      ? matchesEndpoint(readJsonQuietly(home('.claude.json')).mcpServers?.[MCP_SERVER_NAME], endpoint)
      : Object.hasOwn(readJsonQuietly(home('.claude.json')).mcpServers ?? {}, MCP_SERVER_NAME),
  connect: async (e) => {
    await runAgent('claude', ['mcp', 'remove', MCP_SERVER_NAME, '--scope', 'user']).catch(() => {});
    await runAgent('claude', claudeAdd(e));
    allowInClaude(true);
  },
  disconnect: async () => {
    await runAgent('claude', ['mcp', 'remove', MCP_SERVER_NAME, '--scope', 'user']);
    allowInClaude(false);
  },
};

/** Codex's config, where Oya's section goes. */
const codexConfig = () => home('.codex', 'config.toml');

/** Oya's section in Codex's TOML config: its URL, the token, and its tools allowed without a prompt. */
const codexSection = (e: Endpoint) =>
  [
    `\n[mcp_servers.${MCP_SERVER_NAME}]`,
    `url = ${JSON.stringify(e.url)}`,
    `http_headers = { Authorization = ${JSON.stringify(`Bearer ${e.token}`)} }`,
    'default_tools_approval_mode = "approve"\n',
  ].join('\n');

/** Codex: its CLI cannot set a header, so Oya's section is appended after its CLI removes any older one. */
const CODEX: AgentTarget = {
  name: 'Codex',
  installed: () => agentCommand('codex') !== null || fs.existsSync(home('.codex')),
  connected: (endpoint) =>
    fs.existsSync(codexConfig()) &&
    (endpoint
      ? matchesEndpoint(codexEndpoint(fs.readFileSync(codexConfig(), 'utf8')), endpoint)
      : fs.readFileSync(codexConfig(), 'utf8').includes(`[mcp_servers.${MCP_SERVER_NAME}]`)),
  connect: async (e) => {
    if (CODEX.connected()) await CODEX.disconnect();
    writePrivate(codexConfig(), codexSection(e), true);
  },
  disconnect: async () => void (await runAgent('codex', ['mcp', 'remove', MCP_SERVER_NAME])),
};

/** Cursor's entry for Oya's port. */
const cursorEntry = (e: Endpoint): object => ({ url: e.url, headers: auth(e) });

/** VS Code's entry for Oya's port. */
const vscodeEntry = (e: Endpoint): object => ({ type: 'http', url: e.url, headers: auth(e) });

/** Every agent Oya knows how to connect, by id. */
export const AGENTS: Readonly<Record<string, AgentTarget>> = {
  'claude-code': CLAUDE_CODE,
  codex: CODEX,
  cursor: jsonAgent('Cursor', () => home('.cursor', 'mcp.json'), 'mcpServers', cursorEntry),
  vscode: jsonAgent('VS Code', () => vscodeConfig(), 'servers', vscodeEntry),
};

/** The installed agents, and whether each lists Oya. */
export function installedAgents(): AgentStatus[] {
  return Object.entries(AGENTS)
    .filter(([, agent]) => agent.installed())
    .map(([id, agent]) => ({ id, name: agent.name, connected: agent.connected() }));
}
