/**
 * Unit tests for the project model policy: a provider the project allows goes
 * through, one it does not is refused with 403 and audited, and a key with no
 * project keeps every provider.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { requireLlmAllowed, allowedLlm, vendorOf } = await import('../../../../src/modules/agent/llm-policy.ts');
const { recent } = await import('../../../../src/platform/audit.ts');
const { scratchService } = await import('../../support/control.ts');

const KEY = 'llm-policy-key';
const ANTHROPIC = 'https://api.anthropic.com/v1';
const OPENAI = 'https://api.openai.com/v1';
let service;

beforeEach(async () => {
  service = scratchService();
  await service.settings(KEY, { llm: { allow: ['openai'] } });
});

describe('requireLlmAllowed', () => {
  it('lets through a provider the project allows', async () => {
    await assert.doesNotReject(requireLlmAllowed(KEY, OPENAI, service));
  });

  it('refuses a provider the project does not allow, with 403 llm_not_allowed', async () => {
    await assert.rejects(requireLlmAllowed(KEY, ANTHROPIC, service), { status: 403, code: 'llm_not_allowed' });
  });

  it('audits the refusal with the provider it kept out', async () => {
    await requireLlmAllowed(KEY, ANTHROPIC, service).catch(() => null);
    const [row] = recent({ action: 'llm.refused' });
    assert.equal(row.outcome, 'denied');
    assert.deepEqual(row.meta, { provider: 'anthropic' });
  });

  it('refuses every provider when the project allows none', async () => {
    await service.settings(KEY, { llm: { allow: [] } });
    await assert.rejects(requireLlmAllowed(KEY, OPENAI, service), { status: 403 });
  });

  it('allows every provider once the policy is cleared to null', async () => {
    await service.settings(KEY, { llm: null });
    await assert.doesNotReject(requireLlmAllowed(KEY, ANTHROPIC, service));
  });

  it('allows every provider for a key with no project, as on self-host or the CLI', async () => {
    await assert.doesNotReject(requireLlmAllowed('no-project-key', ANTHROPIC, service));
  });

  it('does not create a project for a key that has none', async () => {
    await requireLlmAllowed('still-no-project', ANTHROPIC, service);
    assert.equal((await service.store.list('project', {})).length, 1);
  });
});

describe('vendorOf', () => {
  it('names the vendor by the host the content goes to, not the protocol it speaks', () => {
    assert.equal(vendorOf(ANTHROPIC), 'anthropic');
    assert.equal(vendorOf(OPENAI), 'openai');
    assert.equal(vendorOf('https://generativelanguage.googleapis.com/v1beta/openai'), 'gemini');
    assert.equal(vendorOf('https://aiplatform.googleapis.com/v1/publishers/google'), 'gemini');
    assert.equal(vendorOf('https://openrouter.ai/api/v1'), 'other (openrouter.ai)');
    assert.equal(vendorOf('not a url'), 'other (unknown host)');
  });
});

describe('the policy by vendor', () => {
  it('refuses Gemini and OpenRouter on a project that allows only OpenAI, though both speak its protocol', async () => {
    for (const base of ['https://generativelanguage.googleapis.com/v1beta/openai', 'https://openrouter.ai/api/v1'])
      await assert.rejects(requireLlmAllowed(KEY, base, service), { status: 403 });
  });

  it('lets a Gemini AI Studio key through on a project that allows only Gemini', async () => {
    await service.settings(KEY, { llm: { allow: ['gemini'] } });
    await assert.doesNotReject(
      requireLlmAllowed(KEY, 'https://generativelanguage.googleapis.com/v1beta/openai', service),
    );
  });
});

describe('allowedLlm', () => {
  it('keeps an allowed model', async () => {
    const llm = { openaiKey: 'sk', baseUrl: OPENAI };
    assert.equal(await allowedLlm(KEY, llm, service), llm);
  });

  it('drops a refused model to null, so the work goes on without it', async () => {
    assert.equal(await allowedLlm(KEY, { openaiKey: 'sk', baseUrl: ANTHROPIC }, service), null);
  });
});
