/**
 * Oya's five colours, and the checker every part's design-system test uses to hold its
 * sources to them. The desktop shell, the public site and the console each keep their
 * tokens in their own stylesheet; what may appear there, and how a mix of the five reads
 * for contrast, is decided once, here, so the three cannot drift into three palettes.
 */

/** The five, by the name each part declares it under: ink and paper change places between themes. */
export const FIVE = {
  ink: '#1a1a1a',
  paper: '#f7f5f0',
  lime: '#cdf01e',
  red: '#f2554a',
  amber: '#f5b14c',
};

/** The least a text colour may stand out from what is behind it (WCAG AA for body text). */
export const READABLE = 4.5;

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
/**
 * A hex as a script or document writes a colour: six or eight digits anywhere, three or four only
 * as a value of its own (`'#fff'`, `: #fff`), so a private field `#add` or an id `#feed-list` is not one.
 */
const WRITTEN_HEX = /#(?:[0-9a-f]{8}|[0-9a-f]{6})(?![\w-])|(?<=['"`:]\s*)#[0-9a-f]{3,4}(?=\s*['"`;<}])/gi;

/** `text` with every match of `pattern` turned to spaces, so offsets and line numbers hold. */
const blank = (text, pattern) => text.replace(pattern, (m) => m.replace(/[^\n]/g, ' '));

/** The 1-based line of `offset` in `text`. */
export const lineAt = (text, offset) => text.slice(0, offset).split('\n').length;

/** A stylesheet with its comments, quoted strings and url() bodies blanked. */
export function cssCode(source) {
  const uncommented = blank(source, /\/\*[\s\S]*?\*\//g);
  return blank(blank(uncommented, /url\([^)]*\)/g), /(["'])(?:\\.|(?!\1)[^\\\n])*\1/g);
}

/** The raw colours in one declaration's value; `#000` is let through for a mask. */
function rawColours(property, value) {
  const named = TAKES_COLOUR.test(property) ? [...value.matchAll(NAME)] : [];
  const hits = [...value.matchAll(HEX), ...value.matchAll(FUNCTION), ...named];
  return hits.filter((hit) => !(MASK.test(property) && /^#000$/i.test(hit[0])));
}

/** The colours inside images embedded as data: URLs, which the declaration scan blanks with every other url(). */
function embeddedColours(source, file) {
  const code = blank(source, /\/\*[\s\S]*?\*\//g);
  return [...code.matchAll(/url\(\s*["']?data:[^)]*\)/g)].flatMap((image) =>
    [...image[0].matchAll(EMBEDDED)].map((hit) => `${file}:${lineAt(code, image.index + hit.index)}: ${hit[0]}`),
  );
}

/** Every raw colour in a stylesheet, as `file:line: literal`. */
export function violations(source, file) {
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
function scriptCode(source) {
  const uncommented = blank(blank(source, /\/\*[\s\S]*?\*\/|<!--[\s\S]*?-->/g), /(?<![:'"`])\/\/[^\n]*/g);
  return blank(uncommented, /&#x?[0-9a-f]+;/gi);
}

/** Every hex or colour function in a script or document, as `file:line: literal`. Names are prose there. */
export function literalColours(source, file) {
  const code = scriptCode(source);
  const hits = [...code.matchAll(WRITTEN_HEX), ...code.matchAll(FUNCTION)];
  return hits.map((hit) => `${file}:${lineAt(code, hit.index)}: ${hit[0]}`);
}

/** Whether a hex is one of the five, optionally carrying its own alpha (a canvas cannot mix: `#cdf01e14`). */
export function ofFive(hex) {
  const value = hex.toLowerCase();
  return /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/.test(value) && Object.values(FIVE).includes(value.slice(0, 7));
}

/** Every rule of a stylesheet, as its selector, the line its block opens on, and its declarations by property. */
export function rules(source) {
  const code = cssCode(source);
  // Selectors are read from the source: blanking quoted strings would turn [data-state='done'] into [data-state= ].
  return [...code.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((rule) => ({
    selector: source
      .slice(rule.index, rule.index + rule[1].length)
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .trim()
      .replace(/\s+/g, ' '),
    line: lineAt(code, rule.index + rule[1].length),
    declarations: new Map([...rule[2].matchAll(/([\w-]+)\s*:\s*([^;]+)/g)].map((d) => [d[1], d[2].trim()])),
  }));
}

/** The custom properties the blocks of `css` whose selector matches `selector` declare, later ones winning. */
export function declared(css, selector) {
  const values = new Map();
  for (const block of css.matchAll(/([^{}]+)\{([^{}]*)\}/g))
    if (selector.test(block[1].trim()))
      for (const [, name, value] of block[2].matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) values.set(name, value.trim());
  return values;
}

/** The arguments of a function call, split at its top-level commas. */
function splitArguments(text) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '(') depth++;
    if (text[i] === ')') depth--;
    if (text[i] === ',' && depth === 0) parts.push(text.slice(start, (start = i + 1) - 1));
  }
  return [...parts, text.slice(start)].map((part) => part.trim());
}

/** Two colours mixed as `color-mix(in srgb, a p%, b)` mixes them: alpha first, then each channel by its share of it. */
function mixed(a, share, b) {
  const alpha = a[3] * share + b[3] * (1 - share);
  const channel = (i) => (alpha ? (a[i] * a[3] * share + b[i] * b[3] * (1 - share)) / alpha : 0);
  return [channel(0), channel(1), channel(2), alpha];
}

/** Fails a resolution with what could not be read. */
function unreadable(what) {
  throw new Error(what);
}

/** What a token's value comes to: a hex, `transparent`, another token, or a mix of those, as [r, g, b, alpha]. */
export function resolve(value, tokens) {
  const text = value.trim();
  const hex = /^#([0-9a-f]{6})$/i.exec(text);
  if (hex) return [0, 2, 4].map((i) => parseInt(hex[1].slice(i, i + 2), 16)).concat(1);
  if (text === 'transparent') return [0, 0, 0, 0];
  const token = /^var\((--[\w-]+)\)$/.exec(text);
  if (token) return resolve(tokens.get(token[1]) ?? unreadable(`${token[1]} is not defined`), tokens);
  const mix = /^color-mix\(in srgb,([\s\S]+)\)$/.exec(text) ?? unreadable(`cannot read the colour ${text}`);
  const [first, second] = splitArguments(mix[1]);
  const share = /^([\s\S]+?)\s+([\d.]+)%$/.exec(first) ?? unreadable(`cannot read the share in ${text}`);
  return mixed(resolve(share[1], tokens), Number(share[2]) / 100, resolve(second, tokens));
}

/** `over` laid on an opaque `under`. */
const laid = (over, under) => mixed([over[0], over[1], over[2], 1], over[3], under);

/** A colour's relative luminance (WCAG 2.1). */
function luminance([r, g, b]) {
  const linear = (c) => (c / 255 <= 0.03928 ? c / 255 / 12.92 : ((c / 255 + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** How far token `text` stands out from token `behind`, itself laid on token `under` when it is a translucent tint. */
export function contrast(tokens, text, behind, under) {
  const ground = laid(resolve(`var(${behind})`, tokens), resolve(`var(${under})`, tokens));
  const levels = [luminance(laid(resolve(`var(${text})`, tokens), ground)), luminance(ground)];
  const [light, dark] = levels.sort((a, b) => b - a);
  return (light + 0.05) / (dark + 0.05);
}
