/** Apple owns pointing; Oya retains request lifetime, exact candidate confirmation and list fallback. */
import type { BrowserWindow } from 'electron';
import type { NativePickerPresentation } from '../../shared/share-picker.ts';
import type { NativeWindowSelector } from './native-window-selector.ts';
import type { NativeWindowResult, NativeWindowIdentity } from './native-window-selector.ts';
import { nativeWindowId, type NativeWindowChoice } from './native-window-choice.ts';

/** Main-owned optional selector and one request's candidate catalog. */
export interface NativeShareServices {
  /** One process-wide adapter retains native cancellation quarantine. */
  selector: NativeWindowSelector;
  /** Exact identities and native sources never enter renderer IPC. */
  choice: NativeWindowChoice;
}

/** At most one OS selection is active for a sheet; closing it revokes late callbacks permanently. */
export class NativeShareFlow {
  /** The Oya confirmation surface is hidden while Apple's picker owns the screen. */
  private readonly window: BrowserWindow;
  /** Request-scoped candidate and process-wide native admission. */
  private readonly services: NativeShareServices;
  /** Original document, profile and ownership predicate. */
  private readonly current: () => boolean;
  /** Native cancellation ends the website's current display request. */
  private readonly cancel: () => void;
  /** Shared with the adapter so closing the sheet revokes its callback. */
  private readonly controller = new AbortController();
  /** Prevent duplicate launches and intermediate UI intents during native selection. */
  private busy = false;
  /** A queued card click cannot bypass a subsequent native selection or confirmation. */
  acceptsList = true;
  /** Dependencies are native objects, never identifiers supplied by a page. */
  constructor(window: BrowserWindow, services: NativeShareServices, current: () => boolean, cancel: () => void) {
    this.window = window;
    this.services = services;
    this.current = current;
    this.cancel = cancel;
  }
  /** Clear any prior choice before letting the OS select an actual external window. */
  async choose(): Promise<NativePickerPresentation> {
    if (!this.live() || this.busy) return { status: 'ended' };
    this.services.choice.retire();
    this.busy = true;
    this.acceptsList = false;
    return this.select().finally(() => this.selected());
  }
  /** Native completion can restore the owned sheet only while the original requester remains valid. */
  private selected(): void {
    this.busy = false;
    if (this.live()) this.reveal();
  }
  /** Exclude only this helper; legitimate Oya browser windows remain selectable. */
  private async select(): Promise<NativePickerPresentation> {
    const excluded = nativeWindowId(this.window.getMediaSourceId());
    this.window.hide();
    const result = await this.services.selector.select(excluded === null ? [] : [excluded], this.controller.signal);
    return this.receive(result);
  }
  /** Apple cancellation ends the whole attempt; failure preserves only deliberate list fallback. */
  private receive(result: NativeWindowResult): Promise<NativePickerPresentation> | NativePickerPresentation {
    if (!this.live()) return { status: 'ended' };
    if (result.status === 'cancelled') {
      this.cancel();
      return { status: 'ended' };
    }
    return result.status === 'selected' ? this.offer(result) : this.list(this.failure(result.status));
  }
  /** Showing a candidate does not confirm it or create a stream. */
  private async offer(identity: NativeWindowIdentity): Promise<NativePickerPresentation> {
    const source = await this.services.choice.offer(identity);
    if (!this.live()) return { status: 'ended' };
    return source
      ? { status: 'confirmation', source }
      : this.list("Oya couldn't use the selected window. Nothing was shared.");
  }
  /** A list transition during an outstanding native operation cancels rather than racing its eventual result. */
  back(): NativePickerPresentation {
    if (this.busy) {
      this.cancel();
      return { status: 'ended' };
    }
    return this.list();
  }
  /** Final confirmation performs another exact lookup; a vanished source remains an explicit recoverable error. */
  async confirm(id: string): Promise<boolean> {
    if (!this.live() || this.busy || this.acceptsList) return false;
    this.busy = true;
    try {
      return !!(await this.services.choice.resolve(id)) && this.live();
    } finally {
      this.busy = false;
    }
  }
  /** Errors disclose only the available recovery, never an unproven permission diagnosis. */
  private failure(status: 'failed' | 'unavailable'): string {
    return status === 'failed'
      ? "Couldn't open window selection. Choose a window from the list or try again."
      : 'On-screen window selection is temporarily unavailable. You can still choose a window from the list.';
  }
  /** Returning to cards retires the candidate; it never restores an earlier selected radio button. */
  list(message = ''): NativePickerPresentation {
    this.services.choice.retire();
    this.acceptsList = true;
    return { status: 'list', availability: this.services.selector.state(), message };
  }
  /** A late native result cannot reopen a closed sheet or focus a replacement page. */
  private live(): boolean {
    return !this.controller.signal.aborted && !this.window.isDestroyed() && this.current();
  }
  /** Restore the owned Oya confirmation surface without switching the request to a newly focused tab. */
  private reveal(): void {
    this.window.show();
    this.window.focus();
  }
  /** Every sheet exit revokes the OS callback; native ownership clears only on its terminal acknowledgment. */
  dispose(): void {
    this.controller.abort();
  }
}
