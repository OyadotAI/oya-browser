/**
 * Unit tests for the system prompt: a section for each kind of task value,
 * secrets by placeholder only, and the request_human tool.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { systemPrompt, REQUEST_HUMAN } from '../../../../src/modules/agent/prompt.ts';

const FILE = { file: 'cv.pdf', type: 'application/pdf', b64: 'A'.repeat(4096) };

describe('systemPrompt', () => {
  it('is just the standing instructions for a task with no values', () => {
    const prompt = systemPrompt({}, {}, {}, {});
    assert.match(prompt, /^You are a web automation agent/);
    assert.doesNotMatch(prompt, /TASK VALUES:|^DATA |^FILES you|^SECRETS /m);
  });

  it('lists readable data by placeholder with its value', () => {
    const prompt = systemPrompt({ email: 'a@b' }, { email: 'a@b' }, {}, {});
    assert.match(prompt, /TASK VALUES:/);
    assert.match(prompt, /DATA \(you can read these to decide what to do\):\n\{\{email\}\} = "a@b"/);
  });

  it('lists files with their name, type and size', () => {
    const prompt = systemPrompt({}, {}, { cv: FILE }, {});
    assert.match(prompt, / {2}cv — "cv\.pdf" \(application\/pdf, 3 KB\)/);
  });

  it('shows large files in MB', () => {
    const big = { ...FILE, b64: 'A'.repeat(4 * 1024 * 1024) };
    assert.match(systemPrompt({}, {}, { cv: big }, {}), /3\.0 MB/);
  });

  it('names secrets by placeholder and never includes their values', () => {
    const prompt = systemPrompt({ pw: 'hunter2' }, {}, {}, { pw: 'hunter2' });
    assert.match(prompt, /SECRETS \(hidden from you\): \{\{pw\}\}/);
    assert.doesNotMatch(prompt, /hunter2/);
  });
});

it('offers request_human as a function tool that takes a message', () => {
  assert.equal(REQUEST_HUMAN.function.name, 'request_human');
  assert.deepEqual(REQUEST_HUMAN.function.parameters.required, ['message']);
});
