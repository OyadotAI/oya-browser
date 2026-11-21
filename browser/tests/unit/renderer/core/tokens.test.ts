/**
 * The design system's guard: every colour in the desktop shell comes from core/tokens.css.
 * The token file's own header already said so, and about seventy raw values in twenty
 * stylesheets ignored it, because nothing checked. This reads the sources and fails, naming
 * file and line, on a raw colour, on a `var()` that falls back to one, and on a token that
 * is not defined anywhere (which is how `var(--info, #6cb4ff)` hid).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

const RENDERER = path.resolve(import.meta.dirname, '../../../../src/renderer');
const TOKENS = 'core/tokens.css';

/**
 * Every CSS named colour. The system colours (Canvas, ButtonText and the rest) are left out:
 * in a forced-colours rule they are the only right answer, and they follow the OS, not a brand.
 */
const NAMED = (
  'aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown ' +
  'burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue ' +
  'darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange ' +
  'darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise ' +
  'darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia ' +
  'gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ' +
  'ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan ' +
  'lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue ' +
  'lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon ' +
  'mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue ' +
  'mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin ' +
  'navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen ' +
  'paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red ' +
  'rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue ' +
  'slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white ' +
  'whitesmoke yellow yellowgreen'
).split(' ');

const HEX = /#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})(?![\w-])/gi;
/** A colour function with numbers in it; one that starts `from var(` only reshapes a token. */
const FUNCTION = /(?<![\w.-])(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\((?!\s*from\s+var\()/gi;
/** A hex inside an image written into the stylesheet, where `#` is spelled `%23`. */
const EMBEDDED = /(?:#|%23)(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})(?![\w-])/gi;
/** Properties that take a colour. Names are checked only here: `animation: highlight` is not a colour. */
const TAKES_COLOUR = new RegExp(
  '^(?:--[\\w-]+|(?:[\\w-]+-)?color|background(?:-image)?|(?:border|outline|column-rule|text-decoration|' +
    'text-emphasis|-webkit-text-stroke)(?:-[\\w-]+)?|(?:box|text)-shadow|(?:backdrop-)?filter|fill|stroke)$',
  'i',
);
const NAME = new RegExp(`(?<![\\w-])(?:${NAMED.join('|')})(?![\\w-(])`, 'gi');
/** An alpha mask is drawn in black; there it is a shape, not a colour. */
const MASK = /^(?:-webkit-)?mask(?:-image)?$/;
/** Custom properties the shell sets from TypeScript at run time, so no stylesheet defines them. */
const RUNTIME_PROPERTIES = ['--footer-height', '--panel-reveal', '--row', '--tab-width'];

/**
 * Files outside the stylesheets that still hold raw colours, each with its reason. A file here
 * must still have one, so the list can only shrink.
 */
const EXCEPTIONS: Readonly<Record<string, string>> = {
  'features/connection/components/brand-marks.tsx': "third-party marks: Claude's and Gemini's own colours",
  'features/connection/components/way-icons.tsx': "third-party mark: Google's own gradient",
  'control-shield/constants.ts': 'a separate document that does not load tokens.css yet',
  'control-shield/index.html': 'a separate document that does not load tokens.css yet',
  'public/share-picker.html': 'a separate document that does not load tokens.css yet',
  'public/native-dialog.html': 'a separate document that does not load tokens.css yet',
};

/** `text` with every match of `pattern` turned to spaces, so offsets and line numbers hold. */
const blank = (text: string, pattern: RegExp): string => text.replace(pattern, (m) => m.replace(/[^\n]/g, ' '));

/** The 1-based line of `offset` in `text`. */
const lineAt = (text: string, offset: number): number => text.slice(0, offset).split('\n').length;

/** A stylesheet with its comments, quoted strings and url() bodies blanked. */
function cssCode(source: string): string {
  const uncommented = blank(source, /\/\*[\s\S]*?\*\//g);
  return blank(blank(uncommented, /url\([^)]*\)/g), /(["'])(?:\\.|(?!\1)[^\\\n])*\1/g);
}

/** The raw colours in one declaration's value; `#000` is let through for a mask. */
function rawColours(property: string, value: string): RegExpExecArray[] {
  const named = TAKES_COLOUR.test(property) ? [...value.matchAll(NAME)] : [];
  const hits = [...value.matchAll(HEX), ...value.matchAll(FUNCTION), ...named];
  return hits.filter((hit) => !(MASK.test(property) && /^#000$/i.test(hit[0])));
}

/** The colours inside images embedded as data: URLs, which the declaration scan blanks with every other url(). */
function embeddedColours(source: string, file: string): string[] {
  const code = blank(source, /\/\*[\s\S]*?\*\//g);
  return [...code.matchAll(/url\(\s*["']?data:[^)]*\)/g)].flatMap((image) =>
    [...image[0].matchAll(EMBEDDED)].map((hit) => `${file}:${lineAt(code, image.index + hit.index)}: ${hit[0]}`),
  );
}

/** Every raw colour in a stylesheet, as `file:line: literal`. */
export function violations(source: string, file: string): string[] {
  const code = cssCode(source);
  const failures = embeddedColours(source, file);
  for (const declaration of code.matchAll(/([\w-]+)\s*:\s*([^;{}]+)(?=[;}])/g)) {
    const [whole, property, value] = declaration;
    const start = declaration.index + whole.length - value.length;
    for (const hit of rawColours(property, value))
      failures.push(`${file}:${lineAt(code, start + hit.index)}: ${hit[0].replace(/\($/, '()')}`);
  }
  return failures;
}

/** A script or document with its comments and character references blanked: `// see #1234`, `&#160;`. */
function scriptCode(source: string): string {
  const uncommented = blank(blank(source, /\/\*[\s\S]*?\*\/|<!--[\s\S]*?-->/g), /(?<![:'"`])\/\/[^\n]*/g);
  return blank(uncommented, /&#x?[0-9a-f]+;/gi);
}

/**
 * A hex as a script or document writes a colour: six or eight digits anywhere, three or four only
 * as a value of its own (`'#fff'`, `: #fff`), so a private field `#add` or an id `#feed-list` is not one.
 */
const WRITTEN_HEX = /#(?:[0-9a-f]{8}|[0-9a-f]{6})(?![\w-])|(?<=['"`:]\s*)#[0-9a-f]{3,4}(?=\s*['"`;<}])/gi;

/** Every hex or colour function in a script or document, as `file:line: literal`. Names are prose there. */
export function literalColours(source: string, file: string): string[] {
  const code = scriptCode(source);
  const hits = [...code.matchAll(WRITTEN_HEX), ...code.matchAll(FUNCTION)];
  return hits.map((hit) => `${file}:${lineAt(code, hit.index)}: ${hit[0]}`);
}

/** The renderer's files with one of `extensions`, relative to it, with forward slashes. */
function files(extensions: string[]): string[] {
  const all = readdirSync(RENDERER, { recursive: true }).map((name) => String(name).split(path.sep).join('/'));
  return all.filter((name) => extensions.some((extension) => name.endsWith(extension))).sort();
}

const read = (file: string): string => readFileSync(path.join(RENDERER, file), 'utf8');
const sheets = files(['.css']);

describe('the checker itself', () => {
  it('flags every way of writing a raw colour', () => {
    const raw = ['color: #123456', 'box-shadow: 0 1px rgb(0 0 0 / .2)', 'color: hsla(1, 2%, 3%, 1)'];
    raw.push('color: oklch(60% 0.1 20)', 'color: red', 'color: var(--x, #fff)', 'background: #000');
    raw.push(`background: url("data:image/svg+xml,%3Csvg stroke='%23888'/%3E")`, 'border-top: 1px solid white');
    raw.push('filter: drop-shadow(0 0 2px black)', 'text-decoration: underline red', 'column-rule: 1px solid red');
    raw.push('-webkit-text-stroke: 1px red', 'COLOR: RED');
    for (const declaration of raw)
      assert.equal(violations(`a { ${declaration}; }`, 'x.css').length, 1, `${declaration} is a raw colour`);
  });

  it('names the file and the line', () => {
    assert.deepEqual(violations('a {\n  top: 0;\n  color: #123456;\n}', 'x.css'), ['x.css:3: #123456']);
  });

  it('lets through tokens, mixes of tokens, the two keywords, a mask drawn in black, and comments', () => {
    const fine = ['color: var(--accent)', 'color: color-mix(in srgb, var(--accent) 12%, transparent)'];
    fine.push('color: currentColor', 'mask: linear-gradient(#000, transparent)', 'top: 0 /* was #123456 */');
    fine.push("content: 'red #fff'", 'background: url(a.svg#fff000)', 'white-space: nowrap');
    fine.push('animation: highlight 1s', 'grid-area: mark', 'color: rgb(from var(--accent) r g b / 50%)');
    fine.push('color: CanvasText');
    for (const declaration of fine)
      assert.deepEqual(violations(`a { ${declaration}; }`, 'x.css'), [], `${declaration} is not a raw colour`);
  });
});

describe('the checker, on scripts and documents', () => {
  it('flags a colour written as a value', () => {
    const raw = ["const c = '#39ed35';", "style={{ color: '#fff' }}", '<stop stop-color="#4285f4" />'];
    raw.push("const veil = 'rgba(0, 0, 0, 0.5)';", '<style>a { color: #abc; }</style>');
    for (const line of raw) assert.equal(literalColours(line, 'x.ts').length, 1, `${line} holds a raw colour`);
  });

  it('lets through what only looks like one', () => {
    const fine = [
      '// see #1234 and #abcdef',
      '<p>a&#160;b</p>',
      'this.#add(1);',
      'document.querySelector("#feed-list")',
    ];
    fine.push('chart.color(1)', '<!-- was #ffffff -->', "const url = 'https://a.test/#/abc';");
    for (const line of fine) assert.deepEqual(literalColours(line, 'x.ts'), [], `${line} holds no raw colour`);
  });
});

describe('the shell and its colours', () => {
  it('writes no raw colour in a stylesheet: every one comes from core/tokens.css', () => {
    const failures = sheets.filter((file) => file !== TOKENS).flatMap((file) => violations(read(file), file));
    assert.deepEqual(failures, []);
  });

  it('uses no token that is not defined', () => {
    const css = sheets.map((file) => cssCode(read(file))).join('\n');
    const defined = new Set([...css.matchAll(/(--[\w-]+)\s*:/g)].map((match) => match[1]));
    const used = new Set([...css.matchAll(/var\(\s*(--[\w-]+)/g)].map((match) => match[1]));
    const unknown = [...used].filter((name) => !defined.has(name) && !RUNTIME_PROPERTIES.includes(name));
    assert.deepEqual(unknown.sort(), []);
  });

  it('writes no raw colour in a component, script or document, outside the named exceptions', () => {
    const sources = files(['.ts', '.tsx', '.js', '.html']).filter((file) => !Object.hasOwn(EXCEPTIONS, file));
    assert.deepEqual(
      sources.flatMap((file) => literalColours(read(file), file)),
      [],
    );
  });

  it('keeps no exception for a file that no longer needs one', () => {
    const stale = Object.keys(EXCEPTIONS).filter((file) => literalColours(read(file), file).length === 0);
    assert.deepEqual(stale, []);
  });
});
