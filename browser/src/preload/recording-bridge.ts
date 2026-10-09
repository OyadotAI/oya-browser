/** A narrow isolated-world capability sends recording batches, never arbitrary IPC channels. */
import { NATIVE_RECORDING } from '../shared/native-recording.ts';
/** The only authority this bridge holds is delivery to its fixed recording channel. */
export type RecordingSend = (channel: string, epoch: string, payload: string) => void;
/** One-way messages return no browser-process objects to a renderer. */
export function sendRecording(send: RecordingSend, epoch: unknown, payload: unknown): void {
  if (typeof epoch !== 'string' || !epoch || typeof payload !== 'string') throw new Error('Invalid recording batch');
  if (epoch.length > NATIVE_RECORDING.MAX_EPOCH_CHARS) throw new Error('Recording epoch is too large');
  if (payload.length > NATIVE_RECORDING.MAX_PAYLOAD_CHARS) throw new Error('Recording batch is too large');
  send(NATIVE_RECORDING.CHANNEL, epoch, payload);
}
