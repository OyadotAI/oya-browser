/**
 * The MCP tools that run through the agent's own tool code rather than a single
 * browser command: choosing an option, the console and network logs, reading with
 * a script, waiting, finding, hovering, history, the challenges a site puts up,
 * saved playbooks, and handing a whole task to Oya's agent. Each description and
 * input schema is the one the agent is offered (agent/tools.ts), so the two
 * surfaces cannot drift apart.
 */
import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import {
  BROWSER_TOOLS,
  CHALLENGE_HANDLERS,
  CHALLENGE_TOOLS,
  executeTool,
  runChat,
  toolsOn,
} from '../modules/agent/chat.ts';
import { actionsOf } from '../modules/browsers/socket.ts';
import { challengesFor, quietCheckpointFor } from '../modules/playbooks/checkpoint.ts';
import * as playbooks from '../modules/playbooks/service.ts';
import * as keyConfig from '../modules/config/service.ts';
import { registry } from '../modules/browsers/registry.ts';
import { track } from '../modules/telemetry/index.ts';
import { fail, text, attempt } from './replies.ts';
import { JSON_INDENT } from './constants.ts';

/** The agent's browser tools served as they are. */
const PAGE_TOOLS = [
  'select_option',
  'read_console',
  'read_network',
  'run_script',
  'wait_for',
  'find',
  'hover',
  'go_back',
  'go_forward',
  'reload',
];

/** What one of these tools does, given the browser, the key that owns it, and the call's arguments. */
type Run = (browserId: string, apiKey: string, args: any) => Promise<string>;

/** A tool as registered: its description, its argument shape, and how it runs. */
type AgentTool = {
  /** What the model is told the tool does. */
  description: string;
  /** JSON schema of its arguments. */
  parameters: Record<string, any>;
  /** How a call runs. */
  run: Run;
};

/** One of the agent's tool definitions, by name. */
const definition = (name: string) =>
  [...BROWSER_TOOLS, ...CHALLENGE_TOOLS].find((t) => t.function.name === name)!.function;

/** An agent tool served as it is: its words, its schema, and its handler. */
const pageTool = (name: string): AgentTool => ({
  ...definition(name),
  run: (browserId, _key, args) => executeTool(browserId, name, args, {}),
});

/** A challenge tool, run against the persona of the browser's key. */
const challengeTool = (name: string): AgentTool => ({
  ...definition(name),
  run: (browserId, apiKey) => CHALLENGE_HANDLERS[name](challengesFor(apiKey, browserId)),
});

/** An object schema of the given properties. */
const shape = (properties: Record<string, any>, required: string[] = []) => ({
  type: 'object',
  properties,
  required,
  additionalProperties: false,
});

/** The saved playbooks, as the model reads them. */
const listedPlaybooks = (apiKey: string) =>
  JSON.stringify(
    playbooks.list(apiKey).map(({ name, variables, defaults, steps }) => ({ name, variables, defaults, steps })),
    null,
    JSON_INDENT,
  );

/** Replays a saved playbook on the browser, healing a broken step with the agent. */
async function playPlaybook(browserId: string, apiKey: string, { name, variables = {} }) {
  const pb = keyConfig.getPlaybook(apiKey, name);
  if (!pb) return `Error: no playbook named "${name}". list_playbooks shows the saved ones.`;
  const result = await playbooks.play(apiKey, browserId, pb, variables, {
    checkpoint: quietCheckpointFor(apiKey, browserId),
  });
  return JSON.stringify(result, null, JSON_INDENT);
}

/** Hands a whole task to Oya's agent on this browser; its final report comes back. */
async function runTask(browserId: string, apiKey: string, { task, data = {} }) {
  const walls = { challenges: challengesFor(apiKey, browserId), checkpoint: quietCheckpointFor(apiKey, browserId) };
  const result = await runChat(browserId, [{ role: 'user', content: task }], { apiKey, data, ...walls });
  return result.text;
}

/** The tools only MCP callers get: saved playbooks, and handing a whole task to the agent. */
const OWN_TOOLS: Record<string, AgentTool> = {
  list_playbooks: {
    description:
      'List the playbooks saved on this key: recorded flows that replay without a model, with their variables.',
    parameters: shape({}),
    run: async (_id, apiKey) => listedPlaybooks(apiKey),
  },
  run_playbook: {
    description:
      'Replay a saved playbook on this browser with the given variables. Much faster and cheaper than doing the task step by step; a step that no longer fits the page is finished by the agent.',
    parameters: shape(
      {
        name: { type: 'string', description: 'The playbook, from list_playbooks' },
        variables: { type: 'object', description: 'Values for its variables; its defaults fill the rest' },
      },
      ['name'],
    ),
    run: playPlaybook,
  },
  run_task: {
    description:
      "Hand a whole task to Oya's own browser agent on this browser and get its report back (DONE: or FAILED:). Uses this key's configured model; the run can be saved as a playbook afterwards.",
    parameters: shape(
      {
        task: { type: 'string', description: 'What to do, in plain words' },
        data: { type: 'object', description: 'Values the task uses, referred to as {{name}}' },
      },
      ['task'],
    ),
    run: runTask,
  },
};

/** The challenge tools, by the names the agent knows them by. */
const CHALLENGES = ['solve_captcha', 'sign_in', 'complete_mfa'];

/**
 * The tools of this file, by name. Built on first use, not at load: the agent
 * module sits in an import cycle with the MCP servers (a browser's socket closes
 * its MCP server), so its tool list is not there yet while this file loads.
 */
const agentTools = (): Record<string, AgentTool> => ({
  ...Object.fromEntries(PAGE_TOOLS.map((name) => [name, pageTool(name)])),
  ...Object.fromEntries(CHALLENGES.map((name) => [name, challengeTool(name)])),
  ...OWN_TOOLS,
});

/** Every tool this file serves, by name. */
export const AGENT_TOOL_NAMES = [...PAGE_TOOLS, ...CHALLENGES, 'list_playbooks', 'run_playbook', 'run_task'];

/** Runs a tool on the picked browser; an `Error:` answer, or a throw, is a failed call. */
async function call(name: string, browserId: string, args) {
  const apiKey = registry.get(browserId)?.apiKey ?? '';
  track.mcpToolCalled(apiKey, { tool: name });
  const done = await attempt(() => agentTools()[name].run(browserId, apiKey, args));
  if ('error' in done) return fail(done.error?.message || String(done.error));
  return done.value.startsWith('Error: ') ? fail(done.value.slice('Error: '.length)) : text(done.value);
}

/** A tool's argument shape, as the MCP server takes it. */
const shapeOf = (parameters: Record<string, any>) => (z.fromJSONSchema(parameters) as z.ZodObject<any>).shape;

/** Where a call goes: the browser `pick` names, or the refusal when there is none. */
type Target = {
  /** The browser a call goes to, or null when there is none. */
  pick: () => string | null | undefined;
  /** Error text when there is no browser. */
  noBrowser: string;
  /** Whether pick always names one browser, so the tools it cannot run can be left out. A pool's pick moves on, so it is not asked. */
  oneBrowser?: boolean;
};

/** The MCP handler for a tool: run on the picked browser, or refuse. */
const handlerFor =
  (name: string, { pick, noBrowser }: Target) =>
  async (args) => {
    const id = pick();
    return id ? call(name, id, args) : fail(noBrowser);
  };

/**
 * Whether a tool can run on the browser the target picks now: a page tool the
 * browser does not do (a CDP browser keeps no console log) is left out, as the
 * agent leaves it out, rather than offered and refused.
 */
function runsOn(name: string, target: Target) {
  if (!target.oneBrowser || !PAGE_TOOLS.includes(name)) return true;
  const id = target.pick();
  return toolsOn(id ? actionsOf(id) : null).some((t) => t.function.name === name);
}

/** Registers the named tools of this file that the target's browser can run. */
export function registerAgentTools(server: McpServer, names: string[], target: Target) {
  const tools = agentTools();
  for (const name of names.filter((n) => Object.hasOwn(tools, n) && runsOn(n, target))) {
    const { description, parameters } = tools[name];
    server.tool(name, description, shapeOf(parameters), handlerFor(name, target));
  }
}
