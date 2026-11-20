/**
 * Connecting a coding agent from a link: `oya://agent-connect?agent=<id>`, which the desktop
 * install line opens so a person need not find the footer's Connect button.
 *
 * Any page in any browser can open an oya:// link, and connecting hands an agent a browser that
 * is signed in as this person. So the link only ever asks: a dialog of the app's own window, which
 * no agent tool can answer, with Cancel as its default. The link carries an agent's name and
 * nothing else; the address and token written into the agent's config are this app's own.
 */
import type { MessageBoxOptions, MessageBoxReturnValue } from 'electron';
import { AGENTS, type AgentTarget, type Endpoint } from './agents.ts';
import { agentInstallation, agentPlatform } from './agent-platform.ts';
import {
  CONNECT_LINK_CANCEL_QUIET_MS,
  CONNECT_LINK_COOLDOWN_MS,
  CONNECT_LINK_POLL_MS,
  CONNECT_LINK_PORT_WAIT_MS,
} from './constants.ts';

/** The host of a link that asks to connect an agent. */
const CONNECT_HOST = 'agent-connect';
/** The scheme every Oya link uses, as URL reports it. */
const OYA_PROTOCOL = 'oya:';
/** The button a person clicks to connect; Cancel is first, so Return and Escape both cancel. */
const CONNECT_CHOICE = 1;

/** What the flow needs from the app, so it can be run without one. */
export interface ConnectLinkDeps {
  /** Shows a dialog on the app's own window. */
  ask(options: MessageBoxOptions): Promise<MessageBoxReturnValue>;
  /** Brings the window forward, so the dialog is in front of the terminal that opened the link. */
  show(): void;
  /** This app's MCP address and token, or null while the port is not up. */
  endpoint(): Endpoint | null;
  /** Whether agent `id` is installed on this computer. */
  installed?(id: string): boolean;
  /** The agents that can be connected. */
  agents?: Readonly<Record<string, AgentTarget>>;
}

/** Whether `raw` is a link that asks to connect an agent. */
export function isAgentConnectLink(raw: string): boolean {
  const url = URL.parse(raw);
  return url?.protocol === OYA_PROTOCOL && url.host === CONNECT_HOST;
}

/** A dialog with one OK button that says what happened. */
const told = (message: string, detail: string): MessageBoxOptions => ({
  type: 'info',
  message,
  detail,
  buttons: ['OK'],
});

/** The consent dialog: what connecting grants, with Cancel the default so only a deliberate click connects. */
const consent = (name: string): MessageBoxOptions => ({
  type: 'question',
  message: `Connect ${name} to Oya Browser?`,
  detail: `${name} will work in its own tab of this browser, which is signed in as you. You can take control at any time.`,
  buttons: ['Cancel', `Connect ${name}`],
  ...{ defaultId: 0, cancelId: 0, noLink: true },
});

/** What a person reads when the connection could not be made. */
const failed = (name: string, why: string): MessageBoxOptions =>
  told(`Could not connect ${name}`, `${why} Nothing else was changed. Try Connect next to ${name} in the footer.`);

/** Runs the connect-from-a-link flow, one at a time. */
export class AgentConnectLink {
  /** Whether a flow is in front of the person now. */
  private busy = false;
  /** Until when further links are dropped, so a page opening the link in a loop cannot stack dialogs. */
  private quietUntil = 0;
  /** How long to stay quiet after this flow: longer after the person said no. */
  private quietFor = CONNECT_LINK_COOLDOWN_MS;
  /** The app, as the flow sees it. */
  private readonly deps: ConnectLinkDeps;

  /** `deps` is the app's dialog, window and MCP port. */
  constructor(deps: ConnectLinkDeps) {
    this.deps = deps;
  }

  /** Answers a connect link. A second one while a dialog is up, or just after, is dropped. */
  async open(raw: string): Promise<void> {
    if (this.busy || Date.now() < this.quietUntil) return;
    [this.busy, this.quietFor] = [true, CONNECT_LINK_COOLDOWN_MS];
    try {
      await this.run(this.agentOf(raw));
    } finally {
      this.busy = false;
      this.quietUntil = Date.now() + this.quietFor;
    }
  }

  /** The known agent the link names, or null. The name is looked up, never used as the link gave it. */
  private agentOf(raw: string): string | null {
    const id = URL.parse(raw)?.searchParams.get('agent') ?? '';
    return Object.hasOwn(this.agents, id) ? id : null;
  }

  /** The agents that can be connected. */
  private get agents(): Readonly<Record<string, AgentTarget>> {
    return this.deps.agents ?? AGENTS;
  }

  /** Whether agent `id` is installed on this computer. */
  private installed(id: string): boolean {
    return this.deps.installed ? this.deps.installed(id) : !!agentInstallation(id, agentPlatform());
  }

  /** Brings the window forward and says why nothing can be connected, or goes on to ask. */
  private async run(id: string | null): Promise<void> {
    this.deps.show();
    if (id === null) return void (await this.deps.ask(UNKNOWN_AGENT));
    const agent = this.agents[id];
    if (!this.installed(id)) return void (await this.deps.ask(notInstalled(agent.name)));
    const endpoint = await this.endpoint();
    if (!endpoint) return void (await this.deps.ask(failed(agent.name, 'Oya is not ready to connect agents.')));
    await this.connect(agent, endpoint);
  }

  /** This app's MCP address and token, waited for: a link that launched the app arrives before the port is up. */
  private async endpoint(): Promise<Endpoint | null> {
    const deadline = Date.now() + CONNECT_LINK_PORT_WAIT_MS;
    for (;;) {
      const endpoint = this.deps.endpoint();
      if (endpoint || Date.now() >= deadline) return endpoint;
      await new Promise((resolve) => setTimeout(resolve, CONNECT_LINK_POLL_MS));
    }
  }

  /** Asks the person, and connects only on their click; one already connected is left as it is. */
  private async connect(agent: AgentTarget, endpoint: Endpoint): Promise<void> {
    if (agent.connected(endpoint)) return void (await this.deps.ask(alreadyConnected(agent.name)));
    const answer = await this.deps.ask(consent(agent.name));
    if (answer.response !== CONNECT_CHOICE) return void (this.quietFor = CONNECT_LINK_CANCEL_QUIET_MS);
    const error = await agent.connect(endpoint).then(
      () => null,
      (e: Error) => e,
    );
    await this.deps.ask(error ? failed(agent.name, `${error.message}.`) : connected(agent.name));
  }
}

/** What a person reads when the link names no agent Oya knows. It never repeats the link's own text. */
const UNKNOWN_AGENT = told(
  'This link names an agent Oya does not know',
  `Oya can connect ${Object.values(AGENTS)
    .map((agent) => agent.name)
    .join(', ')}. Nothing was changed.`,
);

/** What a person reads when the agent is not on this computer. */
const notInstalled = (name: string): MessageBoxOptions =>
  told(
    `${name} is not installed on this computer`,
    `Install ${name}, then run the install line again. Nothing was changed.`,
  );

/** What a person reads when the agent already lists this browser. */
const alreadyConnected = (name: string): MessageBoxOptions =>
  told(`${name} is already connected`, 'Nothing was changed. Start a new session if the tools are missing.');

/** What a person reads once the agent is connected. */
const connected = (name: string): MessageBoxOptions =>
  told(
    `${name} is connected`,
    `Start a new ${name} session so the Oya tools appear. If Oya is showing its welcome screen, choose how to start first.`,
  );
