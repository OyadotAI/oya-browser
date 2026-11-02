/**
 * A JSON schema rewritten into the OpenAPI subset Gemini's function declarations take.
 * Vertex and AI Studio answer anything outside that subset with a 400 that fails the
 * whole request, every tool with it, so each schema is rebuilt from the subset rather
 * than having known offenders removed. Where Gemini has no word for a JSON Schema idea,
 * the nearest one is used, and what it loses is written into the description so the
 * model still reads what a value must be.
 */
import { GEMINI_MAX_REF_EXPANSIONS, GEMINI_MAX_SCHEMA_DEPTH, GEMINI_MAX_SCHEMA_NODES } from '../constants.ts';

/** The keywords Gemini's Schema message defines, besides those that hold schemas. */
const KEPT = new Set([
  'type',
  'nullable',
  'description',
  'title',
  'enum',
  'format',
  'pattern',
  'required',
  'minimum',
  'maximum',
  'minItems',
  'maxItems',
  'minLength',
  'maxLength',
  'minProperties',
  'maxProperties',
]);
/** The formats Gemini takes, by type: "only 'enum' and 'date-time' are supported for STRING type". */
const FORMATS = new Map([
  ['string', ['enum', 'date-time']],
  ['number', ['float', 'double']],
  ['integer', ['int32', 'int64']],
]);
/** The keywords that hold schemas, rebuilt the same way. `properties` holds names, so only its values are schemas. */
const CHILDREN = {
  properties: (value, ctx) =>
    Object.fromEntries(Object.entries(isObject(value) ? value : {}).map(([name, s]) => [name, schemaFor(s, ctx)])),
  items: (value, ctx) => schemaFor(value, ctx),
  anyOf: (value, ctx) => value.map((s) => schemaFor(s, ctx)),
};

/** Where a schema sits: what its `$ref`s can point into, and the targets already followed on the way to it. */
type Context = {
  /** The documents a ref resolves against first, innermost first, ending with the tool's own schema. */
  roots: any[];
  /** The schemas enclosing this one, outermost first: where a ref that misses every document is tried next. */
  path: any[];
  /** The ref targets followed on this path, by identity, so a cycle ends instead of looping. */
  seen: Set<any>;
  /**
   * The context each union branch's or allOf member's children were followed in, keyed by
   * the fresh copy that carries them, so a ref one sibling followed is not cut in another.
   */
  owned: WeakMap<object, Context>;
  /** What this whole schema has left to spend, shared by every path, so fan-out cannot grow without end. */
  budget: {
    /** `$ref` expansions still allowed. */
    refs: number;
    /** Schema nodes still allowed; one expansion can copy a large definition, so nodes are charged too. */
    nodes: number;
  };
};

/** Whether a value is a JSON object, the only thing a schema or a ref target can be here. */
const isObject = (value) => Boolean(value) && typeof value === 'object' && !Array.isArray(value);

/** A caller's JSON schema as a Gemini schema; nothing when there is none. */
export const geminiSchema = (schema) =>
  schema === undefined
    ? undefined
    : schemaFor(schema, {
        roots: [schema],
        path: [],
        seen: new Set(),
        owned: new WeakMap(),
        budget: { refs: GEMINI_MAX_REF_EXPANSIONS, nodes: GEMINI_MAX_SCHEMA_NODES },
      });

/** One schema as Gemini's: references and unions flattened, values and type settled, children rebuilt, within the budgets. */
function schemaFor(schema, outer: Context) {
  if (exhausted(outer)) return { type: 'string' };
  const [node, flat] = flattened(schema, ownContext(schema, outer));
  const [united, ctx] = withUnion(node, flat);
  const settled = shaped(withValues(withType(united)));
  const inner = { ...ctx, path: [...ctx.path, schema] };
  const kept = Object.entries(settled).filter(([key]) => KEPT.has(key) || Object.hasOwn(CHILDREN, key));
  return Object.fromEntries(kept.map(([k, v]) => [k, Object.hasOwn(CHILDREN, k) ? CHILDREN[k](v, inner) : v]));
}

/** Whether this schema is past the node or depth budget, which charges it a node when it is not. */
function exhausted(ctx: Context) {
  if (ctx.budget.nodes <= 0 || ctx.path.length >= GEMINI_MAX_SCHEMA_DEPTH) return true;
  ctx.budget.nodes -= 1;
  return false;
}

/** The context a schema is rebuilt in: the one its union branch or allOf member was followed in, where it has one. */
function ownContext(schema, outer: Context): Context {
  const own = isObject(schema) ? outer.owned.get(schema) : undefined;
  return own ? { ...outer, roots: own.roots, seen: own.seen } : outer;
}

/** A schema with its local `$ref` followed and its `allOf` merged in, and the context its children see. */
function flattened(schema, outer: Context): [any, Context] {
  const node = isObject(schema) ? schema : {};
  const [resolved, followed] = followRef(docless(node), withDocument(outer, node));
  return mergeAll(resolved, followed);
}

/**
 * Whether a schema is a document of its own, whose `$ref`s point into it rather than the
 * tool's: return_data nests a caller's extract() schema, `$defs` and all, under `data`.
 */
const isDocument = (node) => Boolean(node.$defs || node.definitions || node.$schema);

/**
 * `ctx` with `node` as the innermost document when it is one. Only an original is ever a
 * root: a copy made while flattening carries no `$defs`, or `#` would name the copy, a new
 * object each time, and a self-reference would never be seen twice.
 */
const withDocument = (ctx: Context, node) => (isDocument(node) ? { ...ctx, roots: [node, ...ctx.roots] } : ctx);

/** A schema without the keywords that make it a document, for a copy that must not become a root. */
const docless = ({ $defs, definitions, $schema, ...body }: any) => body;

/**
 * A `$ref` followed, each target once per path and within the schema's budget. Gemini
 * cannot say "recursive", so a cycle, a ref past the budget, or one that resolves nowhere
 * ends as the schema's own keywords.
 */
function followRef({ $ref, ...rest }: any, ctx: Context): [any, Context] {
  const target = typeof $ref === 'string' ? lookup(ctx, $ref) : undefined;
  if (!target || ctx.seen.has(target) || ctx.budget.refs <= 0) return [rest, ctx];
  ctx.budget.refs -= 1;
  const inner = withDocument({ ...ctx, seen: new Set([...ctx.seen, target]) }, target);
  return followRef({ ...docless(target), ...rest }, inner);
}

/**
 * What a local ref names, tried in order: the documents inside the tool, innermost first;
 * then the schemas enclosing this one, outermost first, below the tool's own; then the
 * tool's own schema. A caller's extract() schema without `$defs` or `$schema` (zod's `#`
 * or `#/properties/home`) points into itself, not the return_data wrapper around it.
 */
const lookup = (ctx: Context, ref: string) =>
  [...ctx.roots.slice(0, -1), ...ctx.path.slice(1), ...ctx.roots.slice(-1)]
    .map((root) => pointer(root, ref))
    .find(isObject);

/** The value a local JSON pointer (`#/$defs/Item`) names in `root`. */
function pointer(root, ref: string) {
  if (!ref.startsWith('#')) return undefined;
  const parts = ref.slice(1).split('/').slice(1);
  return parts.reduce((node, part) => node?.[part.replace(/~1/g, '/').replace(/~0/g, '~')], root);
}

/**
 * Each schema flattened, its children carried in fresh copies that remember the context
 * that schema was followed in. Sharing one context across siblings cut a ref one branch
 * followed in all the others (Pydantic's `Union[Addr, Person]`, Person holding an Addr);
 * dropping it recursed without end (`Optional[Node]` is `anyOf: [{$ref: Node}, null]`).
 */
function flattenedAll(schemas: any[], ctx: Context) {
  return schemas.map((schema) => {
    const [node, inner] = flattened(schema, ownContext(schema, ctx));
    return carried(node, inner);
  });
}

/** The keywords whose schemas `carried` copies: the children, and the unions and tuples converted later. */
const CARRIED = ['items', 'anyOf', 'oneOf', 'prefixItems'];

/** `node` with each child schema replaced by a copy registered to `ctx`, which is where it is rebuilt. */
function carried(node, ctx: Context) {
  const copy = (child) => (isObject(child) ? own(child, ctx) : child);
  const each = (value) => (Array.isArray(value) ? value.map(copy) : copy(value));
  const named = (value) => Object.fromEntries(Object.entries(value).map(([name, child]) => [name, copy(child)]));
  const lists = CARRIED.filter((key) => node[key]).map((key) => [key, each(node[key])]);
  const properties = isObject(node.properties) ? { properties: named(node.properties) } : {};
  return { ...node, ...properties, ...Object.fromEntries(lists) };
}

/** A copy of `child` that is rebuilt in `ctx`; an original document stays the root its refs point into. */
function own(child, ctx: Context) {
  const copy = docless(child);
  ctx.owned.set(copy, withDocument(ctx, child));
  return copy;
}

/** `allOf` merged into one schema, which Gemini has no word for: every member's properties and required names. */
function mergeAll({ allOf, ...rest }: any, ctx: Context): [any, Context] {
  if (!Array.isArray(allOf)) return [rest, ctx];
  return [flattenedAll(allOf, ctx).reduce(merged, rest), ctx];
}

/** Two schemas as one, `first` winning where both say something, their properties and required names combined. */
function merged(first, second) {
  const properties = first.properties || second.properties ? { ...second.properties, ...first.properties } : undefined;
  const required = [...new Set([...(first.required || []), ...(second.required || [])])];
  return { ...second, ...first, ...(properties ? { properties } : {}), ...(required.length ? { required } : {}) };
}

/**
 * `oneOf` as Gemini's `anyOf`, null branches folded into `nullable`, and a union left
 * with one branch collapsed into it. A schema with both keeps `anyOf`; Gemini cannot say
 * "and", so `oneOf`'s constraint is lost there.
 */
function withUnion({ oneOf, anyOf, ...rest }: any, ctx: Context): [any, Context] {
  const branches = Array.isArray(anyOf) ? anyOf : oneOf;
  if (!Array.isArray(branches)) return [rest, ctx];
  const flat = flattenedAll(branches, ctx);
  const real = flat.filter((branch) => !isNull(branch));
  return [union(rest, real, real.length < flat.length), ctx];
}

/** A schema with its non-null branches: kept as anyOf when several, merged in when one or none. */
function union(rest, real: any[], nullable: boolean) {
  const marked = nullable ? { nullable: true } : {};
  return real.length > 1 ? { ...rest, ...marked, anyOf: real } : { ...merged(rest, real[0] || {}), ...marked };
}

/** Whether a schema allows only null. */
const isNull = (schema) => schema.type !== undefined && [schema.type].flat().every((t) => t === 'null');

/**
 * The one type Gemini is told. A `[type, 'null']` list becomes the type and `nullable`.
 * Vertex rejects a schema with no type unless it is a union, so a missing one is
 * inferred from what the schema holds, and is a string when nothing says otherwise.
 */
function withType({ type, ...rest }: any) {
  const types = [type].flat().filter((t) => typeof t === 'string');
  const named = types.find((t) => t !== 'null') ?? (rest.anyOf ? undefined : inferredType(rest));
  return { ...rest, ...(named ? { type: named } : {}), ...(types.includes('null') ? { nullable: true } : {}) };
}

/** The type a schema's contents imply: object from properties, array from items, else its values' type. */
function inferredType(schema) {
  if (schema.properties) return 'object';
  if (schema.items || schema.prefixItems) return 'array';
  return valueType(schema.const ?? schema.enum?.[0]) ?? 'string';
}

/** The Gemini type of one JSON value. */
function valueType(value) {
  if (Array.isArray(value)) return 'array';
  if (Number.isInteger(value)) return 'integer';
  return value === null
    ? undefined
    : { string: 'string', number: 'number', boolean: 'boolean', object: 'object' }[typeof value];
}

/**
 * `const` and `enum` as Gemini takes them. Its enums hold strings only (`enum: [true]` is
 * the TYPE_STRING 400), so other values are written into the description instead.
 */
function withValues({ const: only, enum: values, ...rest }: any) {
  const all = only !== undefined ? [only] : values;
  if (!Array.isArray(all)) return rest;
  if (all.every((v) => typeof v === 'string')) return { ...rest, enum: all };
  const note = `One of: ${all.map((v) => JSON.stringify(v)).join(', ')}.`;
  return { ...rest, description: rest.description ? `${rest.description} ${note}` : note };
}

/** The last fit to Gemini: formats its type takes, one `items` schema for an array, required names that exist. */
function shaped({ format, items, prefixItems, required, ...rest }: any) {
  const names = Object.keys(rest.properties || {});
  return {
    ...rest,
    ...(FORMATS.get(rest.type)?.includes(format) ? { format } : {}),
    ...(rest.type === 'array' ? { items: [items ?? prefixItems ?? {}].flat()[0] ?? {} } : {}),
    ...(Array.isArray(required) ? { required: required.filter((name) => names.includes(name)) } : {}),
  };
}
