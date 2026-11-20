/**
 * Unit tests for prompt line history: masked secret reads never remain in
 * interactive readline history while normal prompt answers do.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const CLI_ROOT = fileURLToPath(new URL('../../..', import.meta.url));

describe('prompt lines secret history', () => {
  it('scrubs masked lines from readline history while keeping unmasked ones', async () => {
    const script = `
      Object.defineProperty(process.stdin, 'isTTY', { value: true, configurable: true });
      Object.defineProperty(process.stdout, 'isTTY', { value: true, configurable: true });
      Object.defineProperty(process.stdout, 'columns', { value: 80, writable: true, configurable: true });
      process.stdin.setRawMode = () => process.stdin;
      process.stdout.cursorTo = () => true;
      process.stdout.clearLine = () => true;
      process.stdout.moveCursor = () => true;
      const { ask, askSecret, closePrompts } = await import('./src/prompt.ts');
      try {
        const secret = await askSecret('STEP1');
        const afterSecret = await ask('STEP2', 'fallback');
        const plain = await ask('STEP3', '');
        const recalled = await ask('STEP4', '');
        process.stderr.write('RESULT:' + JSON.stringify({ secret, afterSecret, plain, recalled }) + '\\n');
      } finally {
        closePrompts();
      }
    `;
    const env = { ...process.env, TERM: 'xterm-256color' };
    delete env.NO_COLOR;
    const child = spawn(process.execPath, ['--input-type=module', '-e', script], {
      cwd: CLI_ROOT,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let out = '';
    let err = '';
    const sent = new Set<string>();
    const replies: Array<[string, string]> = [
      ['STEP1', 'super-secret-key\r'],
      ['STEP2', '\u001b[A\r'],
      ['STEP3', 'visible-label\r'],
      ['STEP4', '\u001b[A\r'],
    ];
    child.stdout.on('data', (chunk: Buffer) => {
      out += chunk.toString();
      for (const [marker, input] of replies) {
        if (out.includes(marker) && !sent.has(marker)) {
          sent.add(marker);
          child.stdin.write(input);
        }
      }
    });
    child.stderr.on('data', (chunk: Buffer) => {
      err += chunk.toString();
    });
    const code = await new Promise<number | null>((resolve) => child.on('close', resolve));
    assert.equal(code, 0, err);
    const match = err.match(/RESULT:(.+)/);
    assert.ok(match, err);
    assert.deepEqual(JSON.parse(match[1]), {
      secret: 'super-secret-key',
      afterSecret: 'fallback',
      plain: 'visible-label',
      recalled: 'visible-label',
    });
    assert.doesNotMatch(out, /super-secret-key/);
  });
});
