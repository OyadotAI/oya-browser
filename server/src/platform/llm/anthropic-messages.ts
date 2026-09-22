/**
 * The shared OpenAI shape to and from Claude's Messages API.
 *
 * - System prompts become one cached system block; tools are cached with it, since
 *   the cache prefix runs tools, then system.
 * - A turn's tool results go back in one user message, which is what keeps Claude
 *   making parallel calls; a screenshot sent after them joins that same message.
 * - Claude's own blocks (thinking included) ride along in the assistant message's
 *   `extra_content.anthropic` and are sent back unchanged for the latest turn, which
 *   is the one a tool loop must repeat. Earlier turns go back as text and tool calls
 *   only, so trimming old history never edits a thinking block.
 */
import type Anthropic from '@anthropic-ai/sdk';

/** Claude's blocks that may be sent back in an assistant turn. */
const REPEATABLE = new Set(['text', 'thinking', 'redacted_thinking', 'tool_use']);

/** A cache breakpoint: the prefix up to here is reused by the next step of the same run. */
const CACHED = { type: 'ephemeral' as const };

/** The system prompts, joined, as one cached block; none when there are none. */
export function systemOf(messages: any[]) {
  const text = messages
    .filter((m) => m.role === 'system' && m.content)
    .map((m) => m.content)
    .join('\n\n');
  return text ? [{ type: 'text' as const, text, cache_control: CACHED }] : undefined;
}

/** OpenAI tool definitions as Claude tools. */
export function toolsOf(tools: any[] = []) {
  return tools.map((t) => ({
    name: t.function.name,
    description: t.function.description,
    input_schema: t.function.parameters || { type: 'object', properties: {} },
  }));
}

/** A base64 data-URL image as a Claude image block, or nothing when it is not one. */
function imageBlock(url: string) {
  const match = /^data:(image\/[\w.+-]+);base64,(.*)$/s.exec(String(url || ''));
  return match ? [{ type: 'image', source: { type: 'base64', media_type: match[1], data: match[2] } }] : [];
}

/** OpenAI user content, a string or text and image parts, as Claude blocks; empty text is dropped (Claude rejects it). */
function userBlocks(content: any): any[] {
  if (!Array.isArray(content)) return content ? [{ type: 'text', text: String(content) }] : [];
  return content.flatMap((part) =>
    part.type === 'image_url' ? imageBlock(part.image_url?.url) : userBlocks(part.text),
  );
}

/** An assistant turn's text and tool calls as Claude blocks. */
function plainAssistant(message: any) {
  const text = message.content ? [{ type: 'text', text: String(message.content) }] : [];
  const calls = (message.tool_calls || []).map((call) => ({
    type: 'tool_use',
    id: call.id,
    name: call.function?.name,
    input: parseInput(call.function?.arguments),
  }));
  return [...text, ...calls];
}

/** A tool call's JSON arguments; empty when they do not parse. */
function parseInput(raw: string) {
  try {
    return JSON.parse(raw || '{}');
  } catch {
    return {};
  }
}

/** The assistant turn's blocks: Claude's own, unchanged, for the latest turn; text and tool calls for earlier ones. */
function assistantBlocks(message: any, latest: boolean) {
  const own = message.extra_content?.anthropic;
  return latest && Array.isArray(own) ? own.filter((b) => REPEATABLE.has(b.type)) : plainAssistant(message);
}

/** Adds `blocks` as a turn of `role`, joining the previous turn when it is the same role (Claude wants turns to alternate). */
function push(turns: any[], role: 'user' | 'assistant', blocks: any[]) {
  if (!blocks.length) return;
  const previous = turns.at(-1);
  if (previous?.role === role) previous.content.push(...blocks);
  else turns.push({ role, content: blocks });
}

/** One OpenAI message's blocks and role, as Claude takes them. */
function turnOf(message: any, latest: boolean): ['user' | 'assistant', any[]] {
  if (message.role === 'assistant') return ['assistant', assistantBlocks(message, latest)];
  if (message.role === 'tool')
    return [
      'user',
      [{ type: 'tool_result', tool_use_id: message.tool_call_id, content: String(message.content ?? '') }],
    ];
  return ['user', userBlocks(message.content)];
}

/** The conversation, system prompts left out, as alternating Claude turns. */
export function messagesOf(messages: any[]): Anthropic.Beta.BetaMessageParam[] {
  const turns: any[] = [];
  const latest = messages.findLastIndex((m) => m.role === 'assistant');
  messages.forEach((m, i) => m.role !== 'system' && push(turns, ...turnOf(m, i === latest)));
  return turns;
}

/** Claude's text blocks, joined. */
const textOf = (content: any[]) =>
  content
    .filter((b) => b.type === 'text')
    .map((b) => b.text)
    .join('');

/** Claude's tool_use blocks as OpenAI tool calls. */
const toolCallsOf = (content: any[]) =>
  content
    .filter((b) => b.type === 'tool_use')
    .map((b) => ({ id: b.id, type: 'function', function: { name: b.name, arguments: JSON.stringify(b.input ?? {}) } }));

/** Everything read (cached and uncached) and everything written, in OpenAI's usage shape. */
function usageOf(usage: any) {
  const read = (usage?.input_tokens || 0) + (usage?.cache_read_input_tokens || 0);
  return {
    prompt_tokens: read + (usage?.cache_creation_input_tokens || 0),
    completion_tokens: usage?.output_tokens || 0,
  };
}

/** What a declined request says: the agent reports it as a failure rather than looping on it. */
const declined = (response: any) =>
  `FAILED: the model declined this request${response.stop_details?.category ? ` (${response.stop_details.category})` : ''}.`;

/** A Claude response in the shared OpenAI shape, its own blocks kept for the next turn. */
export function fromClaude(response: any) {
  const content = response.content || [];
  const calls = toolCallsOf(content);
  const text = response.stop_reason === 'refusal' ? declined(response) : textOf(content) || null;
  const message = { role: 'assistant', content: text, extra_content: { anthropic: content } };
  return {
    choices: [{ message: calls.length ? { ...message, tool_calls: calls } : message }],
    usage: usageOf(response.usage),
  };
}
