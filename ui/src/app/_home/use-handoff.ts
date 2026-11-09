/** A local ownership simulation mirrors takeover, release-to-paused and explicit resumption. */
'use client';
import { useState } from 'react';
import { HANDOFF_EVENT_LIMIT } from './constants';

/** Every simulated owner has one explicit legal transition. */
export type ExampleOwner = 'running' | 'human' | 'paused';
/** Local ownership and a bounded history of synthetic transitions. */
interface ExampleState {
  /** Current simulated ownership. */ owner: ExampleOwner;
  /** Recent ownership events. */ events: string[];
}
/** The event descriptions are separate from fictional hash-chain evidence. */
const TRANSITIONS: Record<
  ExampleOwner,
  { /** Who owns the next state. */ next: ExampleOwner; /** What the example records. */ event: string }
> = {
  running: { next: 'human', event: 'Operator took control. Agent paused.' },
  human: { next: 'paused', event: 'Operator released control. Agent remains paused.' },
  paused: { next: 'running', event: 'Operator explicitly resumed the agent.' },
};

/** The example history is bounded even when a visitor repeats takeover many times. */
export function useHandoff() {
  const [state, setState] = useState<ExampleState>({ owner: 'running', events: [] });
  const advance = () => setState((current) => transition(current));
  const reset = () => setState({ owner: 'running', events: [] });
  return { ...state, advance, reset };
}

/** Only the current state's transition can run; no background activity can resume ownership. */
function transition(current: ExampleState) {
  const next = TRANSITIONS[current.owner];
  return { owner: next.next, events: [...current.events, next.event].slice(-HANDOFF_EVENT_LIMIT) };
}
