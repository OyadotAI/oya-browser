/**
 * Unit tests for argv parsing (src/args.ts).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parse, flagStr, flagNum } from '../../src/args.ts';

describe('parse', () => {
  it('defaults to help with no command', () => {
    assert.deepEqual(parse([]), { command: 'help', args: [], flags: {} });
  });

  it('splits positionals from flags, and a flag takes the next token as its value', () => {
    assert.deepEqual(parse(['goto', 'https://x', '--id', 'b1']), {
      command: 'goto',
      args: ['https://x'],
      flags: { id: 'b1' },
    });
  });

  it('makes a flag followed by another flag, or by nothing, true', () => {
    assert.deepEqual(parse(['rm', '--all', '--json']).flags, { all: true, json: true });
  });

  it('keeps positionals after a flag and its value', () => {
    assert.deepEqual(parse(['personas', '--persona', 'x', 'new', 'n']).args, ['new', 'n']);
  });

  it('a switch never takes the next word as its value, so `stop --force <id>` keeps its id', () => {
    assert.deepEqual(parse(['stop', '--force', 'b1']), { command: 'stop', args: ['b1'], flags: { force: true } });
    assert.deepEqual(parse(['recover', '--replace', 's1']).args, ['s1']);
    assert.deepEqual(parse(['goto', '--json', 'https://x']), {
      command: 'goto',
      args: ['https://x'],
      flags: { json: true },
    });
    assert.deepEqual(parse(['install', '--dry-run', '--help', 'now']).args, ['now']);
  });
});

describe('flag readers', () => {
  it('flagStr returns only string values', () => {
    assert.equal(flagStr({ a: 'x', b: true }, 'a'), 'x');
    assert.equal(flagStr({ a: 'x', b: true }, 'b'), undefined);
  });

  it('flagNum reads a number, or undefined when absent', () => {
    assert.equal(flagNum({ n: '30000' }, 'n'), 30000);
    assert.equal(flagNum({}, 'n'), undefined);
  });
});

describe('the flags the CLI knows', () => {
  it('refuses a flag it does not know, instead of ignoring it', () => {
    assert.throws(() => parse(['start', '--ws_url', 'ws://x']), {
      code: 'usage',
      message: 'Unknown flag --ws_url. Run oya help for the flags.',
    });
    assert.throws(() => parse(['goto', 'x', '--no-captcha']), {
      code: 'usage',
      message: /^Unknown flag --no-captcha\./,
    });
  });

  it('refuses a value flag with no value, naming an example', () => {
    assert.throws(() => parse(['start', '--ws-url']), {
      code: 'usage',
      message: '--ws-url needs a value, such as --ws-url ws://127.0.0.1:9222.',
    });
    assert.throws(() => parse(['start', '--ws-url', '--json']), { code: 'usage' });
  });

  it('refuses a value on a switch, so --all=false never means --all', () => {
    assert.throws(() => parse(['rm', '--all=false']), { code: 'usage', message: '--all takes no value.' });
  });

  it('takes --flag=value, keeping any "=" in the value', () => {
    assert.deepEqual(parse(['ls', '--url=http://127.0.0.1:3100/?a=b']).flags, { url: 'http://127.0.0.1:3100/?a=b' });
  });

  it('leaves the next word alone after a switch, --preview included', () => {
    assert.deepEqual(parse(['personas', 'new', '--preview', 'alice']), {
      command: 'personas',
      args: ['new', 'alice'],
      flags: { preview: true },
    });
  });

  it('refuses a number flag that is not a number, before anything is sent', () => {
    assert.throws(() => flagNum({ 'queue-ms': 'soon' }, 'queue-ms'), {
      code: 'usage',
      message: '--queue-ms must be a number, not "soon".',
    });
  });
});
