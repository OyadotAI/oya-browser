/**
 * The design system's guard: the desktop shell has five colours (tooling/colours), and every
 * colour in it is one of them or a mix of them, taken from core/tokens.css.
 * The token file's own header already said so, and about seventy raw values in twenty
 * stylesheets ignored it, because nothing checked. This reads the sources and fails, naming
 * file and line, on a raw colour, on a `var()` that falls back to one, and on a token that
 * is not defined anywhere (which is how `var(--info, #6cb4ff)` hid).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

import {
  FIVE as SHARED_FIVE,
  READABLE,
  contrast,
  cssCode,
  declared,
  literalColours,
  ofFive,
  rules,
  violations,
} from '../../../../../tooling/colours/index.js';
import { SHELL_BACKGROUND } from '../../../../src/main/shell/constants.ts';
import { DIALOG_WINDOW_OPTIONS } from '../../../../src/main/dialogs/constants.ts';
import { PICKER } from '../../../../src/main/screen-share/constants.ts';

const RENDERER = path.resolve(import.meta.dirname, '../../../../src/renderer');
const TOKENS = 'core/tokens.css';
/** The five colours, by the name tokens.css declares each under, once. */
const FIVE: Readonly<Record<string, string>> = Object.fromEntries(
  Object.entries(SHARED_FIVE).map(([name, hex]) => [`--${name}`, hex]),
);
/** Pages that cannot load tokens.css, so they name the five themselves: nothing but those values may appear. */
const OWN_FIVE = [
  'public/share-picker.html',
  'public/native-dialog.html',
  'control-shield/index.html',
  'control-shield/constants.ts',
];
/** Tokens that are lime, amber or red as they stand: fills, never text, because on paper they cannot be read. */
const FILL_ONLY = /var\(--(?:accent-fill|lime|oya|amber|red|mint|cyan)\)/;

/** Custom properties the shell sets from TypeScript at run time, so no stylesheet defines them. */
const RUNTIME_PROPERTIES = ['--footer-height', '--panel-reveal', '--row', '--tab-width'];

/** The tokens in effect in each theme, as the cascade leaves them on the root element. */
function themes(css: string): Record<'light' | 'dark' | 'incognito', Map<string, string>> {
  const light = declared(css, /^:root$/);
  const dark = new Map([...light, ...declared(css, /^:root\[data-theme='dark'\]$/)]);
  const incognito = new Map([...dark, ...declared(css, /^:root\[data-incognito='true'\]$/)]);
  return { light, dark, incognito };
}

/** Text colours and what each is read against, in every theme. */
const SURFACES = ['--surface', '--bg', '--muted-surface', '--hover'];
const PAIRS: Array<[string, string]> = [
  ...['--text', '--secondary', '--accent', '--danger'].flatMap((text) =>
    SURFACES.map((s): [string, string] => [text, s]),
  ),
  ...SURFACES.slice(0, 3).map((s): [string, string] => ['--warning', s]),
  ['--on-accent', '--accent-fill'],
  ['--on-accent', '--accent-hover'],
  ['--on-danger', '--danger'],
  ['--accent', '--accent-soft'],
  ['--danger', '--danger-soft'],
  ['--warning', '--warning-soft'],
  ['--success', '--success-soft'],
  ['--info', '--info-soft'],
];

/**
 * Files outside the stylesheets that still hold raw colours, each with its reason. A file here
 * must still have one, so the list can only shrink.
 */
const EXCEPTIONS: Readonly<Record<string, string>> = {
  'features/connection/components/brand-marks.tsx': "third-party marks: Claude's and Gemini's own colours",
  'features/connection/components/way-icons.tsx': "third-party mark: Google's own gradient",
};

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

  it('uses no token that is not defined, in a stylesheet, a component or a document', () => {
    const css = sheets.map((file) => cssCode(read(file))).join('\n');
    const markup = files(['.tsx', '.html']).map(read).join('\n');
    // A component sets a property as a quoted style key ('--i': 3) or through setProperty('--i', ...).
    const scripts = files(['.ts', '.tsx']).map(read).join('\n');
    const set = [...scripts.matchAll(/['"](--[\w-]+)['"]\s*:|setProperty\(\s*['"](--[\w-]+)/g)];
    const declares = (text: string): string[] => [...text.matchAll(/(--[\w-]+)\s*:/g)].map((match) => match[1]);
    // The shell's sheets may use only what the shell's sheets declare: a standalone page's own names do not count.
    const forSheets = new Set([...declares(css), ...set.map((match) => match[1] ?? match[2])]);
    const forMarkup = new Set([...forSheets, ...declares(markup)]);
    // A stylesheet may not fall back (a fallback is where a raw colour hid); a document's own optional knob may.
    const inSheets = [...css.matchAll(/var\(\s*(--[\w-]+)/g)].filter((match) => !forSheets.has(match[1]));
    const inMarkup = [...markup.matchAll(/var\(\s*(--[\w-]+)\s*\)/g)].filter((match) => !forMarkup.has(match[1]));
    const used = new Set([...inSheets, ...inMarkup].map((match) => match[1]));
    const unknown = [...used].filter((name) => !RUNTIME_PROPERTIES.includes(name));
    assert.deepEqual(unknown.sort(), []);
  });

  it('writes no raw colour in a component, script or document, outside the named exceptions', () => {
    const named = (file: string): boolean => Object.hasOwn(EXCEPTIONS, file) || OWN_FIVE.includes(file);
    const sources = files(['.ts', '.tsx', '.js', '.html']).filter((file) => !named(file));
    assert.deepEqual(
      sources.flatMap((file) => literalColours(read(file), file)),
      [],
    );
  });

  it('lets a page or canvas that cannot load the tokens name the five, and nothing else', () => {
    // A canvas cannot mix: there a colour of the five may carry its own alpha (#cdf01e14).
    const others = OWN_FIVE.flatMap((file) => literalColours(read(file), file)).filter(
      (hit) => !ofFive(hit.split(': ')[1]),
    );
    assert.deepEqual(others, []);
  });

  it('keeps no exception for a file that no longer needs one', () => {
    const stale = Object.keys(EXCEPTIONS).filter((file) => literalColours(read(file), file).length === 0);
    assert.deepEqual(stale, []);
  });
});

describe('the five colours', () => {
  const css = cssCode(read(TOKENS));

  it('are the only colours tokens.css names, each declared once under its own name', () => {
    const literals = violations(read(TOKENS), TOKENS).map((hit) => hit.split(': ')[1].toLowerCase());
    assert.deepEqual(literals.sort(), Object.values(FIVE).sort());
    for (const [name, hex] of Object.entries(FIVE)) assert.equal(themes(css).light.get(name), hex, `${name} is ${hex}`);
  });

  it('leave no green but lime: the olive accent and the avatar hues are gone', () => {
    const all = sheets.map((file) => cssCode(read(file))).join('\n');
    assert.deepEqual(all.match(/--(?:logo-green|tone-\d)\b/g), null);
  });

  it('match the window the shell is painted into, so nothing flashes at launch', () => {
    assert.deepEqual(SHELL_BACKGROUND, { dark: FIVE['--ink'], light: FIVE['--paper'] });
    assert.equal(DIALOG_WINDOW_OPTIONS.backgroundColor, FIVE['--ink']);
    assert.equal(PICKER.BACKGROUND, FIVE['--ink']);
  });

  it('keep lime, amber and red out of text: on paper they cannot be read', () => {
    const failures = sheets.flatMap((file) =>
      rules(read(file))
        .filter((rule) => FILL_ONLY.test(rule.declarations.get('color') ?? ''))
        .map((rule) => `${file}:${rule.line}: ${rule.selector} { color: ${rule.declarations.get('color')} }`),
    );
    assert.deepEqual(failures, []);
  });

  it('put only ink on a lime or amber fill', () => {
    const fill = /^var\(--(?:accent-fill|accent-hover|lime|amber)\)$/;
    const ink = /^var\(--(?:on-accent|ink)\)$/;
    const failures = sheets.flatMap((file) =>
      rules(read(file))
        .filter((rule) =>
          fill.test(rule.declarations.get('background') ?? rule.declarations.get('background-color') ?? ''),
        )
        .filter((rule) => rule.declarations.has('color') && !ink.test(rule.declarations.get('color') ?? ''))
        .map((rule) => `${file}:${rule.line}: ${rule.selector} { color: ${rule.declarations.get('color')} }`),
    );
    assert.deepEqual(failures, []);
  });

  for (const [theme, tokens] of Object.entries(themes(css)))
    it(`can be read in the ${theme} theme: every text colour stands out from what is behind it`, () => {
      const pairs =
        theme === 'incognito'
          ? [...PAIRS, ...['--text', '--secondary', '--accent'].map((t): [string, string] => [t, '--chrome-bar'])]
          : PAIRS;
      // A control's outline need only stand 3:1 off what is behind it (WCAG 1.4.11).
      const edges = SURFACES.map((surface) => ({
        surface,
        ratio: contrast(tokens, '--control-edge', surface, '--surface'),
      }));
      assert.deepEqual(
        edges
          .filter(({ ratio }) => ratio < 3)
          .map(({ surface, ratio }) => `--control-edge on ${surface}: ${ratio.toFixed(2)}`),
        [],
      );
      const dim = pairs
        .map(([text, behind]) => ({ text, behind, ratio: contrast(tokens, text, behind, '--surface') }))
        .filter(({ ratio }) => ratio < READABLE)
        .map(({ text, behind, ratio }) => `${text} on ${behind}: ${ratio.toFixed(2)}`);
      assert.deepEqual(dim, []);
    });
});
