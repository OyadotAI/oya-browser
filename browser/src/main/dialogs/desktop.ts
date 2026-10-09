/** Route native page decisions to the current human or agent owner without debugging transport. */
import type { WebContents, WebContentsDidStartNavigationEventParams } from 'electron';
import {
  watchNativeDialogs,
  type NativeDialogInfo,
  type NativeDialogReply,
  type NativeDialogPage,
} from '../native/index.ts';
import { DialogService } from './service.ts';
/** Human UI lifetime is independent of the engine reply and may be cancelled during takeover. */
export type PresentDialog = (page: WebContents, info: NativeDialogInfo, reply: NativeDialogReply) => () => void;
/** One native decision keeps its exact page and reply across ownership transfers. */
interface Decision {
  /** Actual source surface, never resolved by URL. */
  page: WebContents;
  /** Untrusted metadata from the engine. */
  info: NativeDialogInfo;
  /** Original one-shot native callback. */
  reply: NativeDialogReply;
  /** Current UI owner; undefined until first routing. */
  human?: boolean;
  /** Close UI without deciding when its owner changes. */
  close?: () => void;
}
/** Native subscriptions apply equally to tabs and OAuth popups in every application mode. */
export class DesktopDialogs extends DialogService {
  /** Exact callbacks are identities, including when URLs or tab ids repeat. */
  private readonly decisions = new Map<NativeDialogReply, Decision>();
  /** Engine subscriptions are installed once and do not retain destroyed pages. */
  private readonly watched = new WeakSet<WebContents>();
  /** Read live input ownership at arrival and immediately before answering. */
  private readonly human: () => boolean;
  /** Browser-owned isolated presentation, never an injected page shim. */
  private readonly present: PresentDialog;
  /** Composition supplies policy, presentation and informational notification delivery. */
  constructor(human: () => boolean, present: PresentDialog, notify?: (message: string, url?: string) => void) {
    super(notify);
    this.human = human;
    this.present = present;
  }
  /** Unsupported engines fail before the page is marked as subscribed. */
  watch(page: WebContents): void {
    if (this.watched.has(page)) return;
    const open = (info: NativeDialogInfo, reply: NativeDialogReply) => this.open(page, info, reply);
    const cancel = (reply: NativeDialogReply) => this.remove(reply);
    const stop = watchNativeDialogs(page as unknown as NativeDialogPage, open, cancel);
    this.watched.add(page);
    page.once('destroyed', stop);
    page.on('did-start-navigation', (event) => this.navigating(page, event));
  }
  /** A replacement document must not wait for a modal sheet belonging to the previous one. */
  private navigating(page: WebContents, event: WebContentsDidStartNavigationEventParams): void {
    if (event.isSameDocument) return;
    for (const decision of [...this.decisions.values()]) {
      if (decision.page !== page || decision.info.dialogType === 'beforeunload') continue;
      if (!event.isMainFrame && event.frame !== decision.info.frame) continue;
      this.remove(decision.reply);
      decision.reply(false);
    }
  }
  /** Transfer still-pending decisions without accepting, rejecting or losing any of them. */
  controlChanged(): void {
    for (const decision of [...this.decisions.values()]) this.route(decision);
  }
  /** Recheck ownership even when a control-change notification has not yet been delivered. */
  override async answer(accept?: boolean, promptText?: unknown) {
    this.controlChanged();
    if (this.human()) return { ok: false, error: 'Human control owns this dialog' };
    return super.answer(accept, promptText);
  }
  /** Alerts remain notification-first; confirmations and prompts always require an explicit owner decision. */
  private open(page: WebContents, info: NativeDialogInfo, reply: NativeDialogReply): void {
    const decision = { page, info, reply };
    this.decisions.set(reply, decision);
    if (info.dialogType === 'alert') this.enqueue(decision);
    else this.route(decision);
  }
  /** Remove the old owner's UI/queue before exposing a decision to the new owner. */
  private route(decision: Decision): void {
    const human = this.human();
    if (human === decision.human) return;
    dismiss(decision);
    this.cancel(decision.reply);
    decision.human = human;
    if (!human) return this.enqueue(decision);
    this.show(decision);
  }
  /** Failed UI creation must cancel safely rather than silently accepting or blocking the renderer forever. */
  private show(decision: Decision): void {
    try {
      const reply: NativeDialogReply = (accept, text) => this.respond(decision, true, accept, text);
      const close = this.present(decision.page, decision.info, reply);
      if (this.decisions.has(decision.reply)) decision.close = close;
      else close();
    } catch {
      this.respond(decision, true, false);
    }
  }
  /** Agent decisions use the same queue and interrupted-command contract as other native dialogs. */
  private enqueue(decision: Decision): void {
    const { info, reply } = decision;
    const answer = (accept: boolean, text?: string) => this.respond(decision, false, accept, text);
    this.receive(queuedDialog(info, reply, answer), info.frame.url);
  }
  /** Stale UI events cannot answer after cancellation, transfer, navigation or destruction. */
  private respond(decision: Decision, human: boolean, accept: boolean, text?: string): void {
    if (!this.decisions.has(decision.reply)) return;
    if (decision.info.dialogType !== 'alert' && this.human() !== human) return this.controlChanged();
    this.remove(decision.reply);
    decision.reply(accept, text);
  }
  /** Engine cancellation only clears the matching surface's decision. */
  private remove(reply: NativeDialogReply): void {
    const decision = this.decisions.get(reply);
    this.decisions.delete(reply);
    this.cancel(reply);
    if (decision) dismiss(decision);
  }
}

/** Shape queue metadata without exposing mutable engine state. */
function queuedDialog(info: NativeDialogInfo, source: NativeDialogReply, reply: NativeDialogReply) {
  return { source, reply, type: info.dialogType, message: info.messageText, defaultPrompt: info.defaultPromptText };
}

/** A transferred presentation is disposed exactly once without consuming the engine reply. */
function dismiss(decision: Decision): void {
  const close = decision.close;
  decision.close = undefined;
  close?.();
}
