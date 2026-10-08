/**
 * The watch-only guard and gate, which act on toolbar controls other features
 * render (Back, Forward, Reload, New tab, a tab's close, Start recording, the
 * address bar), found by id as the legacy shell did:
 * - the guard refuses their clicks in the capture phase, before React's own
 *   listeners, pointing at Take control instead; Start recording takes
 *   control, then presses itself again;
 * - the gate marks them data-control-blocked and aria-disabled, and makes the
 *   address bar read-only.
 */
import { useEffect } from 'react';
import {
  blockedActions,
  type Blocked,
  type ControlState,
  type ControlViewModel,
} from '../view-models/control-view-model.ts';
import { ADDRESS_ID, GUARDED, PAGE_ACTIONS, RECORD_ID } from '../model/constants.ts';

/** The guarded control a click landed on, if any. */
function guardedTarget(event: MouseEvent): HTMLElement | null {
  return event.target instanceof Element ? event.target.closest<HTMLElement>(GUARDED) : null;
}

/** Refuses a click on a page action while watch-only; Start recording takes control first. */
function guardClick(event: MouseEvent, vm: ControlViewModel): void {
  const target = guardedTarget(event);
  if (!target) return;
  const verdict = vm.guard(target.id === RECORD_ID);
  if (verdict === 'allow') return;
  event.preventDefault();
  event.stopImmediatePropagation();
  if (verdict === 'take') void vm.acquire().then((ok) => ok && target.click());
  else vm.requestTakeover();
}

/** Installs the guard on the document while the bar is mounted. */
export function useControlGuard(vm: ControlViewModel): void {
  useEffect(() => {
    const onClick = (event: MouseEvent) => guardClick(event, vm);
    document.addEventListener('click', onClick, true);
    return () => document.removeEventListener('click', onClick, true);
  }, [vm]);
}

/** Marks the page actions blocked (or not) and the address bar read-only, after every change of state. */
export function useControlGate(state: ControlState): void {
  useEffect(() => {
    if (state.control) applyGate(blockedActions(state));
  });
}

/** Marks each page action blocked or not, and the address bar read-only or not. */
function applyGate({ pageBlocked, recordBlocked }: Blocked): void {
  for (const id of PAGE_ACTIONS) {
    const blocked = id === RECORD_ID ? recordBlocked : pageBlocked;
    document.getElementById(id)?.toggleAttribute('data-control-blocked', blocked);
    document.getElementById(id)?.setAttribute('aria-disabled', String(blocked));
  }
  const address = document.getElementById(ADDRESS_ID);
  if (address instanceof HTMLInputElement) address.readOnly = pageBlocked;
}
