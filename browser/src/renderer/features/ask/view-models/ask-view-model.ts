/**
 * The Ask pane: a chat with the agent about the current page. It holds the
 * conversation (messages, and the folded cards of finished runs), sends it
 * with every question, and shows the answer: the reply with its Save as
 * playbook offer, a stop, the model card when the project has no key, or the
 * error. Its collaborators are the live run, the attachments, the profile
 * picker and the setup cards, each its own ViewModel.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { RendererServices } from '../../../app/services.ts';
import { ASK_TEXT, FAILED_REPLY, KEY_CODES, REJECTED_CODE } from '../model/constants.ts';
import type { ChatAnswer, ToolCall } from '../model/types.ts';
import { canReplay, suggestName } from '../model/reply.ts';
import { RunViewModel, type FinishedRun } from './run-view-model.ts';
import { FilesViewModel, readBase64, type ReadFile } from './files-view-model.ts';
import { PersonaViewModel } from './persona-view-model.ts';
import { ModelSetupViewModel } from './model-setup-view-model.ts';
import { PlaybookOfferViewModel } from './playbook-offer-view-model.ts';

/** Who said a message. */
export type Role = 'user' | 'assistant';

/** A message as the agent is sent it (a type, not an interface, so it passes as the bridge's plain payload). */
export type SentMessage = {
  /** Who said it. */
  role: Role;
  /** What was said. */
  content: string;
};

/** One message in the conversation. */
export interface ChatMessage {
  /** Tells a message from a run card. */
  kind: 'message';
  /** Its key in the list. */
  id: number;
  /** Who said it. */
  role: Role;
  /** What was said (Markdown, for the agent). */
  content: string;
  /** The Save as playbook offer under the newest reply, or a saved one; null for none. */
  offer: PlaybookOfferViewModel | null;
}

/** A finished run's card in the conversation. */
export interface RunItem {
  /** Tells a run card from a message. */
  kind: 'run';
  /** Its key in the list. */
  id: number;
  /** The run as it ended. */
  run: FinishedRun;
  /** Its steps are folded behind the summary. */
  folded: boolean;
}

/** One entry of the conversation. */
export type ChatItem = ChatMessage | RunItem;

/** What the Ask pane shows. */
export interface AskState {
  /** A bounded continuation is available after a run reaches its limit. */
  limited: boolean;
  /** The conversation so far. */
  items: ChatItem[];
  /** The Ask box's text. */
  input: string;
  /** A question is on its way: Stop stands in for Send, and the profile cannot change. */
  sending: boolean;
  /** The message whose Copy button says "Copied", or null. */
  copied: number | null;
}

/** What the Ask pane uses: the main process, and the panel (to register Clear and to open on Ask). */
export interface AskServices {
  /** The main process. */
  bridge: RendererServices['bridge'];
  /** The workspace panel. */
  panel: Pick<RendererServices['panel'], 'onClear' | 'open'>;
}

/** The Ask pane. */
export class AskViewModel extends ViewModel<AskState> {
  /** The live run card, the step line and the panel orb. */
  readonly run: RunViewModel;
  /** The attachments. */
  readonly files: FilesViewModel;
  /** The profile picker. */
  readonly persona: PersonaViewModel;
  /** The Sign in and model cards. */
  readonly model: ModelSetupViewModel;
  /** The main process and the panel. */
  private readonly services: AskServices;
  /** Counts Clears, so an answer to a conversation already cleared is dropped. */
  private epoch = 0;
  /** The next item's key. */
  private nextId = 1;
  /** The timer that puts "Copied" back to "Copy". */
  private copyTimer: ReturnType<typeof setTimeout> | undefined;

  /** An empty conversation, registered as the Ask pane's Clear; `readFile` reads attachments (FileReader by default). */
  constructor(services: AskServices, readFile: ReadFile = readBase64) {
    super({ items: [], input: '', sending: false, copied: null, limited: false });
    this.services = services;
    this.run = new RunViewModel(services.bridge);
    this.files = new FilesViewModel(readFile);
    this.persona = new PersonaViewModel(services.bridge);
    this.model = new ModelSetupViewModel(services.bridge, () => void this.turn());
    this.own(services.panel.onClear('chat', () => this.clear()));
    this.own(() => this.disposeParts());
  }

  /** Hands the agent a task (the start page's way in): the panel opens on Ask and sends it. */
  async ask(text: string): Promise<void> {
    const task = text.trim();
    if (!task || this.state.sending) return;
    void this.services.panel.open('chat');
    this.withdraw();
    this.say('user', this.files.attachTo(task));
    await this.turn();
  }

  /** Continues the same task with a new bounded run and the existing conversation. */
  async continueRun(): Promise<void> {
    if (!this.state.limited || this.state.sending) return;
    await this.ask(ASK_TEXT.continueTask);
  }

  /** Sends what is in the Ask box. */
  async send(): Promise<void> {
    const text = this.state.input;
    if (!text.trim() || this.state.sending) return;
    this.set({ input: '' });
    await this.ask(text);
  }

  /** The Ask box's text changed. */
  setInput(input: string): void {
    this.set({ input });
  }

  /** Stops the agent's run: the main process hangs up, and the server ends the run at its next step. */
  stop(): void {
    void this.services.bridge.stopChat();
  }

  /** Forgets the conversation and its files, stopping a run still going. */
  clear(): void {
    if (this.state.sending) this.stop();
    this.epoch++;
    this.withdraw();
    this.files.clear();
    this.run.reset();
    this.set({ items: [], limited: false });
  }

  /** Shows the agent's answer, offering to save the run as a playbook (`replayable` from the server, else judged by its tools). */
  reply(text: string, toolCalls: readonly ToolCall[] = [], replayable?: boolean): void {
    const prompt = this.messages().findLast((m) => m.role === 'user')?.content || '';
    const offer = new PlaybookOfferViewModel(
      this.services.bridge,
      suggestName(prompt),
      replayable ?? canReplay(toolCalls),
    );
    this.withdraw();
    this.say('assistant', text, offer);
  }

  /** A finished run's summary was clicked: opens or folds its steps. */
  toggleRun(id: number): void {
    const flip = (item: ChatItem) => (item.id === id && item.kind === 'run' ? { ...item, folded: !item.folded } : item);
    this.set({ items: this.state.items.map(flip) });
  }

  /** Message `id` was copied: its button says so for a moment. */
  markCopied(id: number): void {
    clearTimeout(this.copyTimer);
    this.set({ copied: id });
    this.copyTimer = setTimeout(() => this.set({ copied: null }), C.COPIED_MS);
  }

  /** Asks with the conversation as it stands, with the input locked until the answer comes; also sends a question again once a model is set. */
  private async turn(): Promise<void> {
    this.set({ sending: true, limited: false });
    this.run.begin();
    const epoch = this.epoch;
    const answer = await this.request();
    if (epoch === this.epoch) this.answer(answer);
    this.set({ sending: false });
  }

  /** The agent's answer to the conversation so far, with its files; a failure as `{ error }`. */
  private async request(): Promise<ChatAnswer> {
    try {
      return ((await this.services.bridge.sendChat(this.messages(), this.files.data())) as ChatAnswer) ?? {};
    } catch (e) {
      return { error: (e as Error).message };
    }
  }

  /** The answer came: the run's card folds into the conversation, then the answer shows. */
  private answer(data: ChatAnswer): void {
    const limited = !!data.limited || data.text === 'Reached iteration limit.';
    this.set({ limited });
    const run = this.run.finish(!data.error && !data.failed && !limited && !FAILED_REPLY.test(data.text || ''));
    if (run) this.set({ items: [...this.state.items, { kind: 'run', id: this.nextId++, run, folded: !!run.summary }] });
    if (data.code && KEY_CODES.includes(data.code)) this.model.needed(data.code === REJECTED_CODE ? data.error : '');
    else if (data.error === ASK_TEXT.stoppedError) this.say('assistant', ASK_TEXT.stopped);
    else if (data.error) this.say('assistant', ASK_TEXT.errorPrefix + data.error);
    else this.reply(data.text || ASK_TEXT.noResponse, data.toolCalls, data.replayable);
  }

  /** Adds one message. */
  private say(role: Role, content: string, offer: PlaybookOfferViewModel | null = null): void {
    this.set({ items: [...this.state.items, { kind: 'message', id: this.nextId++, role, content, offer }] });
  }

  /** The conversation as the agent is sent it. */
  private messages(): SentMessage[] {
    return this.state.items.flatMap((item) =>
      item.kind === 'message' ? [{ role: item.role, content: item.content }] : [],
    );
  }

  /** Removes the unsaved offer: a new run replaces the one the server would save. */
  private withdraw(): void {
    const drop = (item: ChatItem): ChatItem => {
      if (item.kind !== 'message' || !item.offer || item.offer.state.stage === 'saved') return item;
      item.offer.dispose();
      return { ...item, offer: null };
    };
    this.set({ items: this.state.items.map(drop) });
  }

  /** Disposes the collaborators and the Copy timer. */
  private disposeParts(): void {
    clearTimeout(this.copyTimer);
    for (const part of [this.run, this.files, this.persona, this.model]) part.dispose();
  }
}
