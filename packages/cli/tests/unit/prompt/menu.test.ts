/**
 * Unit tests for raw TTY menu key handling (src/prompt/menu.ts): coalesced key
 * bursts (arrows + enter, Ctrl+C inside a burst) and SS3 application-cursor keys.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { stdin, stdout } from 'node:process';
import { choose } from '../../../src/prompt/menu.ts';

const OPTIONS = [
  { id: 'one', label: 'First' },
  { id: 'two', label: 'Second' },
  { id: 'three', label: 'Third' },
];

/** Runs `choose()` with stdin mocked as a TTY and emits raw `chunks` on stdin. */
async function runTty(chunks: string[]) {
  const prevIsTty = Object.getOwnPropertyDescriptor(stdin, 'isTTY');
  const prevIsRaw = Object.getOwnPropertyDescriptor(stdin, 'isRaw');
  const prevSetRaw = (stdin as unknown as Record<string, unknown>).setRawMode;
  const origWrite = stdout.write.bind(stdout);
  Object.defineProperty(stdin, 'isTTY', { value: true, configurable: true });
  Object.defineProperty(stdin, 'isRaw', { value: false, configurable: true });
  (stdin as unknown as Record<string, unknown>).setRawMode = () => stdin;
  (stdout as unknown as Record<string, unknown>).write = () => true;
  try {
    const picked = choose('Pick one', OPTIONS);
    for (const chunk of chunks) stdin.emit('data', Buffer.from(chunk));
    return await picked;
  } finally {
    (stdout as unknown as Record<string, unknown>).write = origWrite;
    if (prevIsTty) Object.defineProperty(stdin, 'isTTY', prevIsTty);
    if (prevIsRaw) Object.defineProperty(stdin, 'isRaw', prevIsRaw);
    (stdin as unknown as Record<string, unknown>).setRawMode = prevSetRaw;
  }
}

describe('TTY menu key handling', () => {
  afterEach(() => stdin.pause());

  it('handles coalesced arrow + enter chunks and application-cursor (SS3) arrows', async () => {
    assert.equal(await runTty(['\x1b[B\r']), 'two');
    assert.equal(await runTty(['\x1bOB\x1bOB\r']), 'three');
    assert.equal(await runTty(['\x1bOB\x1bOA\r']), 'one');
  });

  it('honors Ctrl+C even when coalesced with other keys in one chunk', async () => {
    await assert.rejects(runTty(['\x1b[B\u0003']), /Cancelled\./);
  });
});
