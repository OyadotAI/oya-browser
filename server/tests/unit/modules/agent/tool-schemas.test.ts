/**
 * Every tool the agent can offer, sent the way each model provider receives it, checked
 * against that provider's documented schema rules. One bad schema fails the whole
 * request on Gemini (a boolean `enum: [true]` did, for every run), so this holds the
 * full tool list to every provider at once rather than each tool's own test.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BROWSER_TOOLS, CHALLENGE_TOOLS } from '../../../../src/modules/agent/chat.ts';
import {
  REMEMBER,
  REQUEST_HUMAN,
  RESTART_RECORDING,
  UPDATE_PLAN,
  returnDataTool,
} from '../../../../src/modules/agent/prompt.ts';
import { toGemini } from '../../../../src/platform/llm.ts';
import { toolsOf as claudeTools } from '../../../../src/platform/llm/anthropic-messages.ts';
import { toolsOf as responsesTools } from '../../../../src/platform/llm/responses-messages.ts';

/** A schema an SDK caller hands `extract()`, written the way zod-to-json-schema and hand-written ones are. */
const CALLER_SCHEMA = {
  type: 'object',
  $schema: 'http://json-schema.org/draft-07/schema#',
  additionalProperties: false,
  properties: {
    url: { type: 'string', format: 'uri' },
    price: { type: ['number', 'null'] },
    kind: { const: 'product' },
    default: { type: 'string' },
    id: { oneOf: [{ type: 'string' }, { type: 'integer' }] },
    tags: { type: 'array', items: { type: 'string' }, uniqueItems: true },
    note: { anyOf: [{ type: 'string' }, { type: 'null' }] },
    version: { const: 2 },
    gone: { type: 'null' },
    pair: { type: 'array', items: [{ type: 'number' }, { type: 'string' }] },
    seller: { $ref: '#/$defs/Seller' },
    both: { allOf: [{ properties: { a: { type: 'string' } } }, { properties: { b: { type: 'number' } } }] },
    either: { anyOf: [{ type: 'string' }, { type: 'number' }], oneOf: [{ type: 'string' }, {}] },
  },
  required: ['url', 'price', 'kind', 'default', 'id', 'tags', 'seller'],
  $defs: { Seller: { type: 'object', properties: { name: { type: 'string' }, parent: { $ref: '#/$defs/Seller' } } } },
};

/** Every tool a run can be offered: the browser's, the loop's own, the challenges, and the caller-shaped return_data. */
const ALL_TOOLS = [
  ...BROWSER_TOOLS,
  ...CHALLENGE_TOOLS,
  UPDATE_PLAN,
  RESTART_RECORDING,
  REQUEST_HUMAN,
  REMEMBER,
  returnDataTool(CALLER_SCHEMA),
];

/** A name every provider takes: OpenAI, Responses and Claude allow [a-zA-Z0-9_-]{1,64}; Gemini must not start with a digit. */
const NAME = /^[a-zA-Z_][a-zA-Z0-9_-]{0,63}$/;

/**
 * The keywords Gemini's Schema message defines (Vertex and AI Studio share it); any other is
 * a 400. Taken from Google's reference, not from gemini-schema.ts, so the two can disagree:
 * https://cloud.google.com/vertex-ai/docs/reference/rest/v1/Schema
 */
const GEMINI_KEYWORDS = new Set([
  'type', 'nullable', 'description', 'title', 'enum', 'format', 'pattern', 'required', 'properties', 'items',
  'anyOf', 'minimum', 'maximum', 'minItems', 'maxItems', 'minLength', 'maxLength', 'minProperties', 'maxProperties',
]); // prettier-ignore
/** Gemini's types, one per schema: a JSON Schema type list is a 400. */
const GEMINI_TYPES = new Set(['object', 'array', 'string', 'number', 'integer', 'boolean']);
/** The formats Gemini takes, by type: "only 'enum' and 'date-time' are supported for STRING type". */
const GEMINI_FORMATS = { string: ['enum', 'date-time'], number: ['float', 'double'], integer: ['int32', 'int64'] };

/** Each schema in `schema`, with its path, depth first. */
function* schemasIn(schema, path = '$') {
  if (!schema || typeof schema !== 'object') return;
  yield [path, schema];
  for (const [name, inner] of Object.entries(schema.properties || {})) yield* schemasIn(inner, `${path}.${name}`);
  for (const [i, inner] of (schema.anyOf || schema.oneOf || []).entries()) yield* schemasIn(inner, `${path}|${i}`);
  if (schema.items) yield* schemasIn(schema.items, `${path}[]`);
}

/** What is wrong with one schema for every provider: required names that do not exist, arrays without items. */
function structuralFaults(path, s) {
  const missing = (s.required || []).filter((name) => !Object.hasOwn(s.properties || {}, name));
  return [
    ...missing.map((name) => `${path}: required '${name}' is not a property`),
    ...(s.type === 'array' && !s.items ? [`${path}: array without items`] : []),
  ];
}

/** What Gemini would reject in one schema. Vertex also wants a type on every schema but a union. */
function geminiFaults(path, s) {
  return [
    ...Object.keys(s)
      .filter((k) => !GEMINI_KEYWORDS.has(k))
      .map((k) => `${path}: keyword '${k}'`),
    ...(s.type === undefined && !s.anyOf ? [`${path}: no type`] : []),
    ...(s.type !== undefined && !GEMINI_TYPES.has(s.type) ? [`${path}: type ${JSON.stringify(s.type)}`] : []),
    ...(s.enum && !s.enum.every((v) => typeof v === 'string') ? [`${path}: enum ${JSON.stringify(s.enum)}`] : []),
    ...(s.format !== undefined && !GEMINI_FORMATS[s.type]?.includes(s.format) ? [`${path}: format '${s.format}'`] : []),
    ...(Array.isArray(s.items) ? [`${path}: tuple items`] : []),
  ];
}

/** Every fault `check` finds in each named schema, labelled with its tool. */
const faultsIn = (named: [string, any][], check) =>
  named.flatMap(([name, schema]) =>
    [...schemasIn(schema)].flatMap(([path, s]) => check(path, s).map((f) => `${name} ${f}`)),
  );

/** The top of a tool's parameters as OpenAI and Claude require it: an object, not a union or an enum. */
const topFaults = (named: [string, any][]) =>
  named
    .filter(([, p]) => p?.type !== 'object' || ['anyOf', 'oneOf', 'allOf', 'enum', 'not'].some((k) => k in p))
    .map(([name]) => name);

describe('every agent tool, as each provider receives it', () => {
  it('names each tool once, in characters every provider accepts', () => {
    const names = ALL_TOOLS.map((t) => t.function.name);
    assert.equal(new Set(names).size, names.length);
    assert.deepEqual(
      names.filter((n) => !NAME.test(n)),
      [],
    );
  });

  it('describes every tool, since the model picks tools by their descriptions', () => {
    assert.deepEqual(
      ALL_TOOLS.filter((t) => !String(t.function.description || '').trim()).map((t) => t.function.name),
      [],
    );
  });

  it('requires only properties that exist and gives every array its items', () => {
    assert.deepEqual(
      faultsIn(
        ALL_TOOLS.map((t) => [t.function.name, t.function.parameters]),
        structuralFaults,
      ),
      [],
    );
  });

  it('sends OpenAI chat completions an object schema at the top of every tool', () => {
    assert.deepEqual(topFaults(ALL_TOOLS.map((t) => [t.function.name, t.function.parameters])), []);
  });

  it('sends the Responses API flat function tools with object schemas', () => {
    const tools = responsesTools(ALL_TOOLS);
    assert.ok(tools.every((t) => t.type === 'function' && NAME.test(t.name)));
    assert.deepEqual(topFaults(tools.map((t) => [t.name, t.parameters])), []);
  });

  it('sends Claude an object input_schema for every tool', () => {
    const tools = claudeTools(ALL_TOOLS);
    assert.equal(tools.length, ALL_TOOLS.length);
    assert.deepEqual(topFaults(tools.map((t) => [t.name, t.input_schema])), []);
  });

  it('sends Gemini only the keywords, types, enums and formats its Schema accepts', () => {
    const declarations = toGemini({ tools: ALL_TOOLS }).tools[0].functionDeclarations;
    assert.equal(declarations.length, ALL_TOOLS.length);
    assert.deepEqual(
      faultsIn(
        declarations.map((d) => [d.name, d.parameters]),
        geminiFaults,
      ),
      [],
    );
  });

  it('keeps every property and required field when it trims a schema for Gemini', () => {
    const declarations = toGemini({ tools: ALL_TOOLS }).tools[0].functionDeclarations;
    assert.deepEqual(
      faultsIn(
        declarations.map((d) => [d.name, d.parameters]),
        structuralFaults,
      ),
      [],
    );
    for (const [i, t] of ALL_TOOLS.entries()) {
      const sent = declarations[i].parameters;
      assert.deepEqual(Object.keys(sent?.properties || {}), Object.keys(t.function.parameters?.properties || {}));
    }
  });
});
