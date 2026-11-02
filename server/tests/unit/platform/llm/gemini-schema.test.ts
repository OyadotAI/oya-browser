/**
 * Unit tests for the Gemini schema rewrite: each JSON Schema shape a tool or a caller's
 * extract() schema can hold, as the OpenAPI subset Gemini's function declarations take,
 * keeping the meaning where Gemini has a word for it and the description where it has not.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { geminiSchema } from '../../../../src/platform/llm/gemini-schema.ts';
import {
  GEMINI_MAX_REF_EXPANSIONS,
  GEMINI_MAX_SCHEMA_DEPTH,
  GEMINI_MAX_SCHEMA_NODES,
} from '../../../../src/platform/constants.ts';

/** The Gemini schema of an object holding `properties`. */
const sent = (properties) => geminiSchema({ type: 'object', properties }).properties;

describe('geminiSchema', () => {
  it('sends nothing for a tool with no schema', () => {
    assert.equal(geminiSchema(undefined), undefined);
  });

  it('drops keywords Gemini does not define, at every depth', () => {
    const schema = {
      type: 'object',
      $schema: 'x',
      additionalProperties: false,
      properties: { n: { type: 'number', default: 1, examples: [2] } },
    };
    assert.deepEqual(geminiSchema(schema), { type: 'object', properties: { n: { type: 'number' } } });
  });

  it('keeps a property whose name is a schema keyword', () => {
    assert.deepEqual(sent({ default: { type: 'string' }, enum: { type: 'string' } }), {
      default: { type: 'string' },
      enum: { type: 'string' },
    });
  });

  it('keeps string enums, the only enums Gemini takes', () => {
    assert.deepEqual(sent({ mode: { type: 'string', enum: ['a', 'b'] } }), {
      mode: { type: 'string', enum: ['a', 'b'] },
    });
  });

  it('writes a non-string enum into the description, keeping the type', () => {
    assert.deepEqual(sent({ confirm: { type: 'boolean', enum: [true], description: 'Ask first.' } }), {
      confirm: { type: 'boolean', description: 'Ask first. One of: true.' },
    });
  });

  it('types an untyped enum from its values', () => {
    assert.deepEqual(sent({ size: { enum: [1, 2] } }), { size: { type: 'integer', description: 'One of: 1, 2.' } });
  });

  it('types an untyped enum of lists as an array and of nulls as a string', () => {
    assert.deepEqual(sent({ list: { enum: [[1], [2]] }, none: { enum: [null] } }), {
      list: { type: 'array', description: 'One of: [1], [2].', items: { type: 'string' } },
      none: { type: 'string', description: 'One of: null.' },
    });
  });

  it('sends a string const as a one-value enum', () => {
    assert.deepEqual(sent({ kind: { const: 'product' } }), { kind: { type: 'string', enum: ['product'] } });
  });

  it('types a non-string const from its value and names it in the description', () => {
    assert.deepEqual(sent({ version: { const: 2 }, on: { const: false } }), {
      version: { type: 'integer', description: 'One of: 2.' },
      on: { type: 'boolean', description: 'One of: false.' },
    });
  });

  it('sends a nullable type list as one type marked nullable', () => {
    assert.deepEqual(sent({ p: { type: ['number', 'null'] } }), { p: { type: 'number', nullable: true } });
  });

  it('sends a type of null alone, or a union of only null, as a nullable string', () => {
    assert.deepEqual(sent({ p: { type: 'null' }, q: { anyOf: [{ type: 'null' }] } }), {
      p: { type: 'string', nullable: true },
      q: { type: 'string', nullable: true },
    });
  });

  it('sends oneOf as anyOf, the union Gemini has', () => {
    assert.deepEqual(sent({ id: { oneOf: [{ type: 'string' }, { type: 'integer' }] } }), {
      id: { anyOf: [{ type: 'string' }, { type: 'integer' }] },
    });
  });

  it('folds a null branch of a union into nullable instead of losing it', () => {
    assert.deepEqual(sent({ id: { anyOf: [{ type: 'string' }, { type: 'integer' }, { type: 'null' }] } }), {
      id: { nullable: true, anyOf: [{ type: 'string' }, { type: 'integer' }] },
    });
  });

  it('collapses a union left with one branch into that branch', () => {
    assert.deepEqual(
      sent({ name: { description: 'Who', anyOf: [{ type: 'string', maxLength: 9 }, { type: 'null' }] } }),
      {
        name: { type: 'string', maxLength: 9, description: 'Who', nullable: true },
      },
    );
  });

  it('keeps anyOf when a schema has both, since Gemini cannot say "and"', () => {
    assert.deepEqual(sent({ x: { anyOf: [{ type: 'string' }, { type: 'number' }], oneOf: [{ minLength: 1 }, {}] } }), {
      x: { anyOf: [{ type: 'string' }, { type: 'number' }] },
    });
  });

  it('merges allOf into one schema with every member’s properties and required names', () => {
    const schema = {
      allOf: [
        { properties: { a: { type: 'string' } }, required: ['a'] },
        { properties: { b: { type: 'number' } }, required: ['b'] },
      ],
    };
    assert.deepEqual(sent({ both: schema }), {
      both: { properties: { a: { type: 'string' }, b: { type: 'number' } }, required: ['a', 'b'], type: 'object' },
    });
  });

  it('follows a local $ref into $defs or definitions', () => {
    const schema = {
      type: 'object',
      properties: { item: { $ref: '#/$defs/Item' }, old: { $ref: '#/definitions/Old', description: 'Legacy' } },
      $defs: { Item: { type: 'object', properties: { sku: { type: 'string' } } } },
      definitions: { Old: { type: 'integer' } },
    };
    assert.deepEqual(geminiSchema(schema).properties, {
      item: { type: 'object', properties: { sku: { type: 'string' } } },
      old: { type: 'integer', description: 'Legacy' },
    });
  });

  it('resolves refs in a nested schema that carries its own $defs against that schema', () => {
    const caller = {
      type: 'object',
      properties: { seller: { $ref: '#/$defs/Seller' } },
      $defs: { Seller: { type: 'object', properties: { name: { type: 'string' } } } },
    };
    assert.deepEqual(sent({ data: caller }).data.properties.seller, {
      type: 'object',
      properties: { name: { type: 'string' } },
    });
  });

  it('ends a recursive $ref after one level, since Gemini cannot say "recursive"', () => {
    const schema = { type: 'object', properties: { child: { $ref: '#' } } };
    assert.deepEqual(geminiSchema(schema), {
      type: 'object',
      properties: { child: { type: 'object', properties: { child: { type: 'string' } } } },
    });
  });

  it('ends a recursive ref reached through a union, the shape Pydantic gives Optional[Node]', () => {
    const node = { type: 'object', properties: { next: { anyOf: [{ $ref: '#/$defs/Node' }, { type: 'null' }] } } };
    const schema = {
      $defs: { Node: node },
      type: 'object',
      properties: { head: { anyOf: [{ $ref: '#/$defs/Node' }, { type: 'null' }] } },
    };
    assert.deepEqual(geminiSchema(schema).properties.head, {
      type: 'object',
      nullable: true,
      properties: { next: { type: 'string', nullable: true } },
    });
  });

  it('ends a recursive ref reached through allOf, nested under data as return_data sends it', () => {
    const caller = {
      $defs: { Node: { type: 'object', properties: { child: { allOf: [{ $ref: '#/$defs/Node' }] } } } },
      $ref: '#/$defs/Node',
    };
    assert.deepEqual(sent({ data: caller }).data, { type: 'object', properties: { child: { type: 'string' } } });
  });

  it('resolves a #/properties ref in a nested schema without $defs against that schema, as zod writes them', () => {
    const caller = {
      type: 'object',
      properties: {
        home: { type: 'object', properties: { city: { type: 'string' } } },
        work: { $ref: '#/properties/home' },
      },
    };
    assert.deepEqual(sent({ data: caller }).data.properties.work, {
      type: 'object',
      properties: { city: { type: 'string' } },
    });
  });

  it('resolves the same $defs name in two documents to each one’s own definition', () => {
    const schema = {
      $defs: { Item: { type: 'integer' } },
      type: 'object',
      properties: {
        outer: { $ref: '#/$defs/Item' },
        data: { $defs: { Item: { type: 'boolean' } }, type: 'object', properties: { inner: { $ref: '#/$defs/Item' } } },
      },
    };
    const out = geminiSchema(schema).properties;
    assert.deepEqual([out.outer, out.data.properties.inner], [{ type: 'integer' }, { type: 'boolean' }]);
  });

  it('sends an object whose properties are not an object with no properties, rather than throwing', () => {
    assert.deepEqual(geminiSchema({ type: 'object', properties: null }), { type: 'object', properties: {} });
  });

  it('stops expanding refs past the budget, so a fan-out of definitions cannot grow to 2^n nodes', () => {
    const depth = 30;
    const $defs = Object.fromEntries(
      Array.from({ length: depth }, (_, i) => [
        `D${i}`,
        { type: 'object', properties: { a: { $ref: `#/$defs/D${i + 1}` }, b: { $ref: `#/$defs/D${i + 1}` } } },
      ]),
    );
    const out = geminiSchema({ $defs, $ref: '#/$defs/D0' });
    const refsLeft = (s) =>
      s?.properties ? Object.values(s.properties).reduce((n: number, p) => n + refsLeft(p), 1) : 1;
    assert.ok(refsLeft(out) <= 2 * GEMINI_MAX_REF_EXPANSIONS + 1);
  });

  it('stops building nodes past the budget, so refs to one large definition grow the output by the input’s size at most, not by the refs', () => {
    const wide = {
      type: 'object',
      properties: Object.fromEntries(Array.from({ length: 400 }, (_, i) => [`f${i}`, { type: 'string' }])),
    };
    const uses = Object.fromEntries(Array.from({ length: 250 }, (_, i) => [`u${i}`, { $ref: '#/$defs/Wide' }]));
    const schema = { $defs: { Wide: wide }, type: 'object', properties: uses };
    const out = geminiSchema(schema);
    const nodes = (s) => 1 + Object.values(s?.properties || {}).reduce((n: number, p) => n + nodes(p), 0);
    const input = nodes(schema) + nodes(wide);
    assert.ok(nodes(out) <= GEMINI_MAX_SCHEMA_NODES + input, `${nodes(out)} nodes from ${input}`);
    assert.deepEqual(out.properties.u249, { type: 'string' });
  });

  /** Pydantic's Addr and a Person holding one, for the sibling-branch rules. */
  const ADDR_AND_PERSON = {
    Addr: { type: 'object', properties: { city: { type: 'string' } } },
    Person: { type: 'object', properties: { name: { type: 'string' }, address: { $ref: '#/$defs/Addr' } } },
  };
  /** What Addr comes out as. */
  const ADDR_OUT = { type: 'object', properties: { city: { type: 'string' } } };

  it('follows a ref in one union branch that a sibling branch already followed, as Union[Addr, Person] needs', () => {
    const schema = {
      $defs: ADDR_AND_PERSON,
      type: 'object',
      properties: { contact: { anyOf: [{ $ref: '#/$defs/Addr' }, { $ref: '#/$defs/Person' }] } },
    };
    assert.deepEqual(geminiSchema(schema).properties.contact.anyOf[1].properties.address, ADDR_OUT);
  });

  it('follows a ref in one allOf member that another member already followed', () => {
    const schema = {
      $defs: ADDR_AND_PERSON,
      type: 'object',
      properties: { x: { allOf: [{ $ref: '#/$defs/Addr' }, { $ref: '#/$defs/Person' }] } },
    };
    assert.deepEqual(geminiSchema(schema).properties.x.properties.address, ADDR_OUT);
  });

  it('resolves a caller’s # against the caller’s schema, not the return_data wrapper around it', () => {
    const caller = {
      type: 'object',
      properties: { name: { type: 'string' }, children: { type: 'array', items: { $ref: '#' } } },
    };
    const out = geminiSchema({ type: 'object', properties: { data: caller }, required: ['data'] }).properties.data;
    assert.deepEqual(Object.keys(out.properties.children.items.properties), ['name', 'children']);
  });

  it('resolves a caller’s field named data against the caller’s schema', () => {
    const caller = { type: 'object', properties: { data: { type: 'integer' }, copy: { $ref: '#/properties/data' } } };
    assert.deepEqual(sent({ data: caller }).data.properties.copy, { type: 'integer' });
  });

  it('expands a union branch that names its whole document once, then stops', () => {
    const schema = {
      $defs: { Addr: ADDR_OUT },
      anyOf: [{ $ref: '#' }, { type: 'object', properties: { addr: { $ref: '#/$defs/Addr' } } }],
    };
    const out = geminiSchema(schema);
    assert.deepEqual(out.anyOf[1].properties.addr, ADDR_OUT);
    assert.ok(JSON.stringify(out).length < JSON.stringify(schema).length * 4, JSON.stringify(out));
  });

  it('ends a self-reference inside a oneOf that a followed branch brought in', () => {
    const out = geminiSchema({ $defs: {}, oneOf: [{ $ref: '#' }, { type: 'string' }] });
    assert.ok(JSON.stringify(out).length < 200, JSON.stringify(out));
  });

  it('keeps the items of union branches, a schema or true alike', () => {
    assert.deepEqual(
      sent({
        x: {
          anyOf: [
            { type: 'array', items: { type: 'number' } },
            { type: 'array', items: true },
          ],
        },
      }),
      {
        x: {
          anyOf: [
            { type: 'array', items: { type: 'number' } },
            { type: 'array', items: { type: 'string' } },
          ],
        },
      },
    );
  });

  it('stops nesting past the depth budget, so a deep chain of items cannot overflow the stack', () => {
    let schema: any = { type: 'string' };
    for (let i = 0; i < 1500; i += 1) schema = { type: 'array', items: schema };
    let out = geminiSchema(schema);
    let depth = 0;
    while (out.items) [out, depth] = [out.items, depth + 1];
    assert.ok(depth <= GEMINI_MAX_SCHEMA_DEPTH);
  });

  it('types a ref it cannot follow as a string rather than failing the request', () => {
    assert.deepEqual(sent({ a: { $ref: 'https://x.test/s.json' }, b: { $ref: '#/$defs/Missing' } }), {
      a: { type: 'string' },
      b: { type: 'string' },
    });
  });

  it('types every schema but a union, inferring from what it holds, since Vertex rejects one without', () => {
    assert.deepEqual(
      sent({ a: { properties: {} }, b: { items: { type: 'string' } }, c: { description: 'x' }, d: true }),
      {
        a: { properties: {}, type: 'object' },
        b: { items: { type: 'string' }, type: 'array' },
        c: { description: 'x', type: 'string' },
        d: { type: 'string' },
      },
    );
  });

  it('keeps a format only on the type Gemini takes it for', () => {
    const properties = {
      url: { type: 'string', format: 'uri' },
      at: { type: 'string', format: 'date-time' },
      n: { type: 'number', format: 'date-time' },
      f: { type: 'number', format: 'float' },
    };
    assert.deepEqual(sent(properties), {
      url: { type: 'string' },
      at: { type: 'string', format: 'date-time' },
      n: { type: 'number' },
      f: { type: 'number', format: 'float' },
    });
  });

  it('sends one items schema for a tuple, an array without items, and prefixItems', () => {
    const properties = {
      pair: { type: 'array', items: [{ type: 'number' }, { type: 'string' }] },
      bare: { type: 'array' },
      prefixed: { type: 'array', prefixItems: [{ type: 'integer' }] },
      empty: { type: 'array', items: [] },
    };
    assert.deepEqual(sent(properties), {
      pair: { type: 'array', items: { type: 'number' } },
      bare: { type: 'array', items: { type: 'string' } },
      prefixed: { type: 'array', items: { type: 'integer' } },
      empty: { type: 'array', items: { type: 'string' } },
    });
  });

  it('requires only properties that exist, since Gemini rejects a required name it cannot find', () => {
    assert.deepEqual(geminiSchema({ type: 'object', properties: { a: { type: 'string' } }, required: ['a', 'gone'] }), {
      type: 'object',
      properties: { a: { type: 'string' } },
      required: ['a'],
    });
  });
});
