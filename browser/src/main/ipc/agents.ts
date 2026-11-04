/**
 * The footer's coding-agent calls: which agents this computer has, and adding
 * Oya's MCP port to one or taking it out. Only the person's click does either.
 */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';
import { AGENTS, knownAgents } from '../local-mcp/index.ts';
import { AGENT_TEST_PROMPT } from '../../shared/agent-setup.ts';
import type { Failure, LocalAgent } from '../../shared/ipc.ts';

/** The services these calls use. */
type Deps = Pick<AppServices, 'localMcp' | 'agentLauncher' | 'electron'>;

/** The channels this group answers. */
type Channel = 'local-agents' | 'connect-agent' | 'disconnect-agent' | 'open-agent' | 'copy-agent-test';

/** The coding-agent handlers. */
export class AgentHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'local-agents': (_event, all) => this.list(all === true),
    'open-agent': (_event, id) => this.open(String(id)),
    'copy-agent-test': (_event, id) => {
      this.requireAgent(String(id));
      this.deps.electron.clipboard.writeText(AGENT_TEST_PROMPT);
    },
    'connect-agent': (_e, id) => this.change(String(id), true),
    'disconnect-agent': (_e, id) => this.change(String(id), false),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The installed agents, or none when this browser offers no port. */
  private list(all = false): LocalAgent[] {
    const mcp = this.deps.localMcp;
    return mcp?.token ? knownAgents({ url: mcp.url, token: mcp.token }, all) : [];
  }

  /** Agent identities cannot be used as arbitrary executable paths or clipboard input. */
  private requireAgent(id: string): void {
    if (!Object.hasOwn(AGENTS, id) || !this.deps.localMcp?.token) throw Error('That agent is unavailable.');
  }

  /** Safe launch errors keep the sample prompt available for a manual retry. */
  private async open(id: string): Promise<Failure> {
    this.requireAgent(id);
    try {
      await this.deps.agentLauncher.open(id);
      return {};
    } catch {
      return { error: 'Could not open this agent. Open it manually, then paste the test prompt.' };
    }
  }

  /** Adds Oya to agent `id` or takes it out; answers the list as it now stands. */
  private async change(id: string, connect: boolean): Promise<LocalAgent[]> {
    const mcp = this.deps.localMcp;
    if (!Object.hasOwn(AGENTS, id) || !mcp?.token) throw new Error('That agent cannot be connected here.');
    if (connect) await AGENTS[id].connect({ url: mcp.url, token: mcp.token });
    else await AGENTS[id].disconnect();
    return this.list();
  }
}
