/**
 * Unit tests for the one action vocabulary: its Oya column equals what the
 * desktop app announces, its CDP column equals the CDP driver's handlers, the
 * published lists hold one spelling and no internal action, a non-string is
 * never an action, and the server's code reads from browser/ only what its
 * image carries.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  VOCABULARY,
  SPELLINGS,
  actionsFor,
  canonical,
  isInternal,
  supportedOn,
} from '../../../src/drivers/vocabulary.ts';
import { HANDLERS } from '../../../src/drivers/cdp/handlers/index.ts';
import { normalise } from '../../../src/drivers/cdp/actions.ts';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '../../../..');
const { OYA_ACTIONS } = createRequire(import.meta.url)('../../../../browser/main/actions/vocabulary.cjs');

describe('the action vocabulary', () => {
  it('has the Oya column the desktop app announces', () => {
    assert.deepEqual(actionsFor('oya'), OYA_ACTIONS);
  });

  it('has the CDP column the CDP driver has handlers for; the dialog answer lives in its dialog guard', () => {
    for (const action of actionsFor('cdp')) {
      if (action === 'handle_dialog') continue;
      const mapped = normalise(action, {}).action;
      assert.ok(Object.hasOwn(HANDLERS, mapped), `${action} -> ${mapped} has no handler`);
    }
    const handled = new Set(Object.keys(HANDLERS).map((name) => canonical(name)));
    for (const action of handled) assert.ok(VOCABULARY[action!]?.cdp, `${action} is handled but not in the CDP column`);
  });

  it('publishes one spelling per action and no internal one', () => {
    for (const kind of ['oya', 'cdp'] as const) {
      const listed = actionsFor(kind);
      assert.deepEqual(listed, [...listed].sort(), kind);
      for (const action of listed) {
        assert.ok(!isInternal(action), `${kind} lists ${action}`);
        assert.ok(!Object.hasOwn(SPELLINGS, action), `${kind} lists the spelling ${action}`);
      }
    }
  });

  it('reads only a string as an action: a wrapped name is not the action it wraps', () => {
    assert.equal(canonical(['evaluate_raw']), null);
    assert.equal(canonical({ toString: () => 'click' }), null);
    assert.equal(isInternal(['evaluate_raw']), false);
    assert.equal(canonical('scroll-down'), 'scroll');
    assert.equal(canonical('toString'), null);
  });

  it('says where an action is supported', () => {
    assert.deepEqual(supportedOn('read_console'), ['oya']);
    assert.deepEqual(supportedOn('back'), ['cdp']);
    assert.deepEqual(supportedOn('click'), ['oya', 'cdp']);
  });

  it('keeps the server image whole: server code reads from browser/ only what the Dockerfile copies', () => {
    const copied = [...readFileSync(join(repo, 'Dockerfile'), 'utf8').matchAll(/^COPY (browser\/\S+)/gm)].map(
      (m) => m[1],
    );
    const src = join(repo, 'server/src');
    const files = (dir: string): string[] =>
      readdirSync(dir).flatMap((f) => (statSync(join(dir, f)).isDirectory() ? files(join(dir, f)) : [join(dir, f)]));
    let checked = 0;
    for (const file of files(src).filter((f) => f.endsWith('.ts'))) {
      for (const [, path] of readFileSync(file, 'utf8').matchAll(/['"]((?:\.\.\/)+browser\/[^'"]+)['"]/g)) {
        const target = relative(repo, join(dirname(file), path));
        assert.ok(
          copied.some((c) => target === c || target.startsWith(c.endsWith('/') ? c : `${c}/`)),
          `${relative(repo, file)} reads ${target}, which the image does not carry`,
        );
        checked++;
      }
    }
    assert.ok(checked > 0, 'found no browser/ path to check: the pattern no longer matches how the server reads it');
  });
});
