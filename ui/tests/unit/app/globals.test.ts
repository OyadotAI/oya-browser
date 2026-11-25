/**
 * The console's design-system guard: the docs, sign-in pages and the console take their colours
 * from globals.css, which names Oya's five (tooling/colours) once and mixes everything else from
 * them. This fails, naming file and line, on any other colour: a raw value, a class from Tailwind's
 * own palette (cleared in globals.css, so such a class would silently draw nothing), a lime tint
 * that loses its edge, and on a text colour or control outline that does not stand out enough.
 */
import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import {
  FIVE,
  READABLE,
  contrast,
  declared,
  lineAt,
  literalColours,
  violations,
} from '../../../../tooling/colours/index.js';

const SRC = path.resolve(import.meta.dirname, '../../../src');
const GLOBALS = 'app/globals.css';
/** The public site's palettes, which name the five themselves (held by tests/unit/app/_home/tokens.test.ts). */
const PUBLIC_PALETTES = ['app/_home/landing.module.css', 'app/page.module.css'];

/** A file under src, read. */
const read = (file: string): string => readFileSync(path.join(SRC, file), 'utf8');

/** Every file under src with one of `extensions`, relative to src, with forward slashes. */
function files(extensions: string[]): string[] {
  const all = readdirSync(SRC, { recursive: true }).map((name) => String(name).split(path.sep).join('/'));
  return all.filter((name) => extensions.some((extension) => name.endsWith(extension))).sort();
}

/** A colour class from Tailwind's own palette, which globals.css clears: it would compile to nothing. */
const PALETTE = new RegExp(
  '(?<![\\w-])(?:[\\w-]+:)*(?:bg|text|border(?:-[trblxyse])?|ring(?:-offset)?|inset-ring|from|via|to|fill|stroke|' +
    'shadow|inset-shadow|drop-shadow|text-shadow|outline|divide|decoration|placeholder|caret|accent)-(?:black|white|' +
    '(?:slate|gray|zinc|neutral|stone|mauve|olive|mist|taupe|red|orange|amber|yellow|lime|green|emerald|teal|cyan|' +
    'sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\\d{2,3})(?![\\w-])',
  'g',
);
/**
 * Classes that undo the design: indigo (info is neutral now), a lime tint by opacity (bg-accent-soft is
 * the tint, and a tint gets no edge), a lime gradient or glow, and a red tint strong enough to sink the
 * red text on it below 4.5:1.
 */
const BANNED =
  /(?<![\w-])(?:[\w-]+:)*(?:[a-z-]+-indigo(?![\w-])|bg-accent\/|(?:from|via|to|shadow)-accent(?![\w-])|bg-red\/(?:[2-9]\d|1[6-9]))/g;

/** Every match of `pattern` in `file`, as `file:line: match`. */
function hits(file: string, pattern: RegExp): string[] {
  const source = read(file);
  return [...source.matchAll(pattern)].map((match) => `${file}:${lineAt(source, match.index)}: ${match[0]}`);
}

/** The tokens in effect in each theme, as the cascade leaves them on the root element. */
function themes(): Record<'dark' | 'light', Map<string, string>> {
  // Comments go, but quoted strings stay: the light block is selected by [data-theme='light'].
  const css = read(GLOBALS).replace(/\/\*[\s\S]*?\*\//g, '');
  // globals.css opens with its @imports, which sit in front of the first block.
  const dark = declared(css, /(?:^|[\s;]):root$/);
  return { dark, light: new Map([...dark, ...declared(css, /(?:^|[\s;]):root\[data-theme='light'\]$/)]) };
}

describe('the console and its colours', () => {
  it('names each of the five once, in globals.css, and no other colour', () => {
    const literals = violations(read(GLOBALS), GLOBALS).map((hit) => hit.split(': ')[1].toLowerCase());
    expect(literals.sort()).toEqual(Object.values(FIVE).sort());
    for (const [name, hex] of Object.entries(FIVE)) expect(themes().dark.get(`--${name}`), name).toBe(hex);
  });

  it('writes no raw colour in any other stylesheet', () => {
    const sheets = files(['.css']).filter((file) => file !== GLOBALS && !PUBLIC_PALETTES.includes(file));
    expect(sheets.flatMap((file) => violations(read(file), file))).toEqual([]);
  });

  it('writes no raw colour in a component or page', () => {
    expect(files(['.ts', '.tsx']).flatMap((file) => literalColours(read(file), file))).toEqual([]);
  });

  it('uses no colour class from Tailwind’s own palette, which is cleared', () => {
    expect(read(GLOBALS)).toMatch(/@theme\s*\{\s*--color-\*:\s*initial;\s*\}/);
    expect(files(['.ts', '.tsx', '.css']).flatMap((file) => hits(file, PALETTE))).toEqual([]);
  });

  it('uses no indigo, no lime tint by opacity, no lime gradient or glow, and no strong red tint', () => {
    expect(files(['.ts', '.tsx']).flatMap((file) => hits(file, BANNED))).toEqual([]);
  });

  it('never puts the page colour on a lime fill', () => {
    const paperOnLime = files(['.ts', '.tsx']).flatMap((file) =>
      [...read(file).matchAll(/(['"`])[^'"`]*\1/g)]
        .filter(
          ([classes]) => /(?<![\w-])bg-accent(?![\w/-])/.test(classes) && /(?<![\w-])text-bg(?![\w-])/.test(classes),
        )
        .map(([classes]) => `${file}: ${classes}`),
    );
    expect(paperOnLime).toEqual([]);
  });

  it('splits the accent: text reads the readable accent, a fill reads lime; small amber and red marks get an ink edge on paper', () => {
    const globals = read(GLOBALS);
    expect(globals).toContain('--color-accent: var(--primary);');
    expect(globals).toContain('--background-color-accent: var(--accent-fill);');
    for (const fill of ['yellow', 'red'])
      expect(globals).toMatch(
        new RegExp(`@utility bg-${fill} \\{\\s*--tw-inset-ring-shadow: inset 0 0 0 1px var\\(--fill-edge\\);`),
      );
    const { light, dark } = themes();
    expect(light.get('--primary')).toBe('var(--ink)');
    expect(light.get('--fill-edge')).toBe('var(--ink)');
    expect(dark.get('--primary-foreground')).toBe('var(--ink)');
  });
});

describe('the console can be read', () => {
  const SURFACES = ['--background', '--card', '--elevated', '--sunken'];
  const TEXT = ['--foreground', '--secondary', '--muted', '--dim', '--primary', '--info', '--warning', '--destructive'];
  /** Text, what it sits on, and what that sits on when it is a translucent tint. */
  const PAIRS: Array<[string, string, string]> = [
    ...TEXT.flatMap((text) => SURFACES.map((surface): [string, string, string] => [text, surface, surface])),
    ['--primary-foreground', '--accent-fill', '--accent-fill'],
    ['--primary-foreground', '--accent-fill-hover', '--accent-fill-hover'],
    ...['--foreground', '--primary', '--dim'].flatMap((text) =>
      SURFACES.map((surface): [string, string, string] => [text, '--accent-soft', surface]),
    ),
  ];

  for (const [theme, tokens] of Object.entries(themes())) {
    /** A tint of `colour` at `share`, laid on each surface, behind `text`. */
    const tinted = (text: string, colour: string, share: number) =>
      SURFACES.map((surface) => {
        tokens.set('--tint', `color-mix(in srgb, var(${colour}) ${share}%, transparent)`);
        return {
          pair: `${text} on ${colour} ${share}% over ${surface}`,
          ratio: contrast(tokens, text, '--tint', surface),
        };
      });

    it(`in the ${theme} theme, every text colour stands out from what is behind it`, () => {
      const measured = [
        ...PAIRS.map(([text, behind, under]) => ({
          pair: `${text} on ${behind}`,
          ratio: contrast(tokens, text, behind, under),
        })),
        ...tinted('--destructive', '--red', 15),
        ...tinted('--warning', '--amber', 10),
      ];
      expect(
        measured.filter(({ ratio }) => ratio < READABLE).map(({ pair, ratio }) => `${pair}: ${ratio.toFixed(2)}`),
      ).toEqual([]);
    });

    it(`in the ${theme} theme, a control's outline stands 3:1 off every surface`, () => {
      const faint = SURFACES.map((surface) => ({
        surface,
        ratio: contrast(tokens, '--border-control', surface, surface),
      }))
        .filter(({ ratio }) => ratio < 3)
        .map(({ surface, ratio }) => `--border-control on ${surface}: ${ratio.toFixed(2)}`);
      expect(faint).toEqual([]);
    });
  }
});
