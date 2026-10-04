/**
 * What the Ask pane's tests share: a pass of the event loop, an Ask pane over
 * a fake bridge and a real panel, and an answer the test releases by hand.
 */
import { AskViewModel } from '../../../../../src/renderer/features/ask/view-models/ask-view-model.ts';
import { PanelViewModel } from '../../../../../src/renderer/app/panel/panel-view-model.ts';
import { fakeBridge, manualFrames } from '../../support/bridge.ts';

/** Every Ask pane a test built, disposed after it so no ticker outlives the test. */
const built: AskViewModel[] = [];

/** Disposes the Ask panes built so far (call from afterEach). */
export const disposeAll = () => built.splice(0).forEach((ask) => ask.dispose());

/** Lets pending promises run. */
export const settle = () => new Promise<void>((resolve) => setImmediate(resolve));

/** A fake File: its base64 rides along. */
interface FakeFile {
  /** The file's bytes, base64. */
  b64: string;
}

/** A file read by the fake reader: its base64 travels on the fake File. */
export const file = (name: string, b64: string, type = 'text/plain') => ({ name, type, b64 }) as unknown as File;

/** Reads a fake file. */
export const fakeRead = async (f: File) => (f as unknown as FakeFile).b64;

/** An Ask pane whose sendChat waits until the test answers it, plus any further `answers`. */
export async function chatting(answers: Record<string, unknown> = {}) {
  const pending: ((data: unknown) => void)[] = [];
  const sendChat = () => new Promise((resolve) => pending.push(resolve));
  const fake = fakeBridge({ sendChat, ...answers });
  const panel = new PanelViewModel(fake.bridge, manualFrames());
  const ask = new AskViewModel({ bridge: fake.bridge, panel }, fakeRead);
  built.push(ask);
  await settle();
  /** Types `text` and sends it. */
  const send = async (text: string) => {
    ask.setInput(text);
    void ask.send();
    await settle();
  };
  /** Answers the oldest question still waiting. */
  const answer = async (data: unknown) => (pending.shift()?.(data), await settle());
  return { fake, panel, ask, send, answer };
}

/** The messages shown, as "role: text". */
export const shown = (ask: AskViewModel) =>
  ask.state.items.flatMap((i) => (i.kind === 'message' ? [`${i.role}: ${i.content}`] : []));
