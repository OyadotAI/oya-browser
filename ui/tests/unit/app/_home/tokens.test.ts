/**
 * The public site's design-system guard: the landing page and the public pages around it
 * (trust, comparisons, research, release notes, legal and the answer pages) use Oya's five
 * colours (tooling/colours) and nothing else. Each palette names the five once and mixes
 * everything else from them; this fails, naming file and line, on any other colour, on lime
 * written as text where it sits on paper (it cannot be read there), and on a text colour
 * that does not stand out from what is behind it.
 */
import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import {
  FIVE,
  READABLE,
  contrast,
  cssCode,
  declared,
  literalColours,
  ofFive,
  rules,
  violations,
} from '../../../../../tooling/colours/index.js';

const APP = path.resolve(import.meta.dirname, '../../../../src/app');
/** The two palettes: the home page's, and the one the other public pages share. Each names the five, once. */
const PALETTES = ['_home/landing.module.css', 'page.module.css'];
/**
 * Every folder of the app: the public pages and, since the console moved onto the five too, the console's
 * (whose palette, globals.css, tests/unit/app/globals.test.ts holds). None may name a colour of its own.
 */
const PUBLIC = readdirSync(APP, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);
/** The favicon: an image the browser draws before any stylesheet, so it names the five itself. */
const ICON = 'icon.svg';
/** How each palette calls the five. */
const NAMES: Readonly<Record<keyof typeof FIVE, string>> = {
  ink: '--night',
  paper: '--ivory',
  lime: '--lime',
  red: '--red',
  amber: '--amber',
};

/**
 * The rules that set lime as text, each on a dark section where it reads at 13:1. Anywhere else lime
 * is text on paper (1.2:1). A new one has to be checked and added; one that is gone has to be removed.
 */
const LIME_TEXT_ON_DARK = [
  '_home/closing.module.css .footer :global(.footer__col a:hover)',
  '_home/closing.module.css .footer :global(.footer__legal a:hover)',
  '_home/control-plane.module.css .arrow',
  '_home/demo.module.css .result svg',
  '_home/hero.module.css .accent',
  '_home/ledger.module.css .verified',
  '_home/ledger.module.css .hash',
  "_home/replay-showcase.module.css .codePane li[data-state='done'] .gutter",
  '_home/replay-showcase.module.css .codePane em',
  '_home/stack.module.css .examples[open] .code',
  '_home/workflow-story.module.css .step svg, .branches svg',
  '_home/workflow-story.module.css .connector',
  '_home/workflow-story.module.css .repair > svg',
  '_answers/answers.module.css .eyebrow',
  '_answers/visuals.module.css .node svg',
  '_answers/visuals.module.css .hub',
  '_answers/visuals.module.css .task',
  '_answers/visuals.module.css .comparisonLabel',
];

/** A file of the app, read. */
const read = (file: string): string => readFileSync(path.join(APP, file), 'utf8');

/** The files under `folder` with one of `extensions`, relative to the app, with forward slashes. */
function files(folder: string, extensions: string[]): string[] {
  const all = readdirSync(path.join(APP, folder), { recursive: true }).map((name) => `${folder}/${String(name)}`);
  return all.map((name) => name.split(path.sep).join('/')).filter((name) => extensions.some((e) => name.endsWith(e)));
}

const sheets = [...PUBLIC.flatMap((folder) => files(folder, ['.css'])), 'page.module.css'];
const code = [...PUBLIC.flatMap((folder) => files(folder, ['.ts', '.tsx'])), 'page.tsx'];

/** A palette's tokens: what its `.page` block declares. */
const tokensOf = (file: string): Map<string, string> => declared(cssCode(read(file)), /^\.page$/);

describe('the public site and its colours', () => {
  it('names each of the five once in each palette, and no other colour', () => {
    for (const file of PALETTES) {
      const literals = violations(read(file), file).map((hit) => hit.split(': ')[1].toLowerCase());
      expect(literals.sort(), file).toEqual(Object.values(FIVE).sort());
      for (const [colour, name] of Object.entries(NAMES))
        expect(tokensOf(file).get(name), `${file} ${name}`).toBe(FIVE[colour as keyof typeof FIVE]);
    }
  });

  it('writes no raw colour in any other stylesheet: every one comes from a palette', () => {
    const others = sheets.filter((file) => !PALETTES.includes(file));
    expect(others.flatMap((file) => violations(read(file), file))).toEqual([]);
  });

  it('writes no raw colour in the pages’ code', () => {
    expect(code.flatMap((file) => literalColours(read(file), file))).toEqual([]);
  });

  it('draws the favicon in the five', () => {
    const others = literalColours(read(ICON), ICON).filter((hit) => !ofFive(hit.split(': ')[1]));
    expect(others).toEqual([]);
    expect(read(ICON)).toContain(FIVE.lime);
  });

  it('writes lime as text only where it sits on a dark section', () => {
    const lime = sheets.flatMap((file) =>
      rules(read(file))
        .filter((rule) => /var\(--lime\)/.test(rule.declarations.get('color') ?? ''))
        .map((rule) => `${file} ${rule.selector}`),
    );
    expect(lime.sort()).toEqual([...LIME_TEXT_ON_DARK].sort());
  });

  it('keeps no accent of its own: the old forest-green one is gone', () => {
    expect(tokensOf(PALETTES[0]).has('--accent')).toBe(false);
  });
});

describe('the palettes can be read', () => {
  /** Text, what it sits on, and what that sits on when it is a translucent tint. */
  const HOME: Array<[string, string, string]> = [
    ['--ink', '--ivory', '--ivory'],
    ['--muted', '--ivory', '--ivory'],
    ['--muted', '--surface', '--ivory'],
    ['--muted', '--surface-soft', '--ivory'],
    ['--primary', '--ivory', '--ivory'],
    ['--ivory', '--night', '--night'],
    ['--night-muted', '--night', '--night'],
    ['--night-muted', '--forest', '--night'],
    ['--lime', '--night', '--night'],
    ['--lime', '--forest', '--night'],
    ['--lime', '--lime-tint', '--night'],
    ['--on-lime', '--lime', '--lime'],
    ['--ink', '--lime-soft', '--ivory'],
  ];
  /** A control's outline, what it sits on, and what that sits on: 3:1 is enough for an outline (WCAG 1.4.11). */
  const EDGES: Array<[string, string, string]> = [
    ['--control-edge', '--ivory', '--ivory'],
    ['--control-edge', '--surface', '--ivory'],
    ['--control-edge', '--surface-soft', '--ivory'],
    ['--dark-control-edge', '--night', '--night'],
    ['--dark-control-edge', '--forest', '--night'],
  ];

  it('_home/landing.module.css: every control outline stands 3:1 off what is behind it', () => {
    const tokens = tokensOf(PALETTES[0]);
    const faint = EDGES.map(([edge, behind, under]) => ({ edge, behind, ratio: contrast(tokens, edge, behind, under) }))
      .filter(({ ratio }) => ratio < 3)
      .map(({ edge, behind, ratio }) => `${edge} on ${behind}: ${ratio.toFixed(2)}`);
    expect(faint).toEqual([]);
  });
  const PAGES: Array<[string, string, string]> = [
    ...['--foreground', '--secondary', '--muted', '--dim', '--primary', '--info', '--warning', '--destructive'].flatMap(
      (text) =>
        ['--background', '--elevated', '--sunken'].map((behind): [string, string, string] => [text, behind, '--ivory']),
    ),
    ['--primary-foreground', '--primary', '--primary'],
    ['--night-muted', '--night', '--night'],
    ['--night-muted', '--forest', '--night'],
    ['--lime', '--forest', '--night'],
  ];

  for (const [file, pairs] of [
    [PALETTES[0], HOME],
    [PALETTES[1], PAGES],
  ] as const)
    it(`${file}: every text colour stands out from what is behind it`, () => {
      const tokens = tokensOf(file);
      const dim = pairs
        .map(([text, behind, under]) => ({ text, behind, ratio: contrast(tokens, text, behind, under) }))
        .filter(({ ratio }) => ratio < READABLE)
        .map(({ text, behind, ratio }) => `${text} on ${behind}: ${ratio.toFixed(2)}`);
      expect(dim).toEqual([]);
    });
});
