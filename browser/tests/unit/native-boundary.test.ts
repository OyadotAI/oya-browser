/** Enforce the migrated native surface without claiming that legacy application modules are CDP-free. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';

const main = path.resolve(import.meta.dirname, '../../src/main');
const directories = readdirSync(main).filter((name) => name === 'native' || name.startsWith('native-'));
directories.push('input', 'dialogs');
const forbiddenImport = /(?:^|\/)(?:cdp|front-door|playwright|puppeteer|chrome-remote-interface)(?:\/|$)/;
const forbiddenProperty = new Set(['debugger', 'sendCommand', 'sendCdp', 'connectOverCDP']);

/** Inspect syntax rather than comments, allowing type-only compatibility aliases but no runtime transport. */
function violations(source: string): string[] {
  const failures: string[] = [];
  const file = ts.createSourceFile('surface.ts', source, ts.ScriptTarget.Latest, true);
  const visit = (node: ts.Node): void => {
    if (ts.isPropertyAccessExpression(node) && forbiddenProperty.has(node.name.text)) failures.push(node.getText(file));
    if (
      ts.isElementAccessExpression(node) &&
      ts.isStringLiteral(node.argumentExpression) &&
      forbiddenProperty.has(node.argumentExpression.text)
    )
      failures.push(node.getText(file));
    if (
      ts.isImportDeclaration(node) &&
      !node.importClause?.isTypeOnly &&
      ts.isStringLiteral(node.moduleSpecifier) &&
      forbiddenImport.test(node.moduleSpecifier.text)
    )
      failures.push(node.getText(file));
    if (
      ts.isExportDeclaration(node) &&
      !node.isTypeOnly &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier) &&
      forbiddenImport.test(node.moduleSpecifier.text)
    )
      failures.push(node.getText(file));
    if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword || node.expression.getText(file) === 'require') &&
      node.arguments.some((arg) => ts.isStringLiteral(arg) && forbiddenImport.test(arg.text))
    )
      failures.push(node.getText(file));
    if (
      ts.isCallExpression(node) &&
      /(?:^|\.)(?:send|sendCommand|command|request)$/.test(node.expression.getText(file)) &&
      node.arguments[0] &&
      ts.isStringLiteral(node.arguments[0]) &&
      /^(?:Runtime|Page|Target|Emulation|Network|Browser)\.[A-Za-z]+$/.test(node.arguments[0].text)
    )
      failures.push(node.getText(file));
    ts.forEachChild(node, visit);
  };
  visit(file);
  return failures;
}

it('rejects protocol dependencies, computed debugger access and dynamic transport imports', () => {
  for (const source of [
    'wc.debugger.attach()',
    'wc["debugger"].attach()',
    'transport.sendCommand("Page.enable")',
    'send("Runtime.enable")',
    'page.send("Page.addScriptToEvaluateOnNewDocument", {source})',
    'import { Cdp } from "../cdp/index.ts"',
    'export { Cdp } from "../cdp/index.ts"',
    'await import("playwright")',
    'require("puppeteer")',
  ])
    assert.notEqual(violations(source).length, 0, source);
  assert.deepEqual(violations('import type { PageView } from "../cdp/cdp.ts"; // debugger is forbidden'), []);
});

for (const directory of directories) {
  for (const file of readdirSync(path.join(main, directory), { recursive: true })
    .map(String)
    .filter((name) => name.endsWith('.ts'))) {
    it(`${directory}/${file} has no internal protocol dependency`, () => {
      assert.deepEqual(violations(readFileSync(path.join(main, directory, file), 'utf8')), []);
    });
  }
}

for (const file of [
  'connection/cdp-relay.ts',
  'connection/native-screenshot.ts',
  'recording/channels.ts',
  'recording/native-channel.ts',
  'recording/native-frames.ts',
  'recording/native-output.ts',
  'workflow/native-validation.ts',
  'workflow/native-preflight.ts',
  'workflow/native-actions.ts',
  'workflow/native-target.ts',
  'workflow/native-run-targets.ts',
  'workflow/native-report.ts',
]) {
  it(`${file} keeps its migrated native boundary`, () => {
    assert.deepEqual(violations(readFileSync(path.join(main, file), 'utf8')), []);
  });
}

/** Type-only aliases cannot bring legacy transports into the bundled application. */
function runtimeSpecifiers(source: string): string[] {
  const result: string[] = [];
  const file = ts.createSourceFile('module.ts', source, ts.ScriptTarget.Latest, true);
  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node) && !node.importClause?.isTypeOnly && ts.isStringLiteral(node.moduleSpecifier)) {
      const bindings = node.importClause?.namedBindings;
      const onlyNamedTypes =
        !node.importClause?.name &&
        bindings &&
        ts.isNamedImports(bindings) &&
        bindings.elements.every((binding) => binding.isTypeOnly);
      if (!onlyNamedTypes) result.push(node.moduleSpecifier.text);
    }
    if (
      ts.isExportDeclaration(node) &&
      !node.isTypeOnly &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier)
    ) {
      const onlyTypes =
        node.exportClause &&
        ts.isNamedExports(node.exportClause) &&
        node.exportClause.elements.every((binding) => binding.isTypeOnly);
      if (!onlyTypes) result.push(node.moduleSpecifier.text);
    }
    if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword || node.expression.getText(file) === 'require') &&
      node.arguments[0] &&
      ts.isStringLiteral(node.arguments[0])
    )
      result.push(node.arguments[0].text);
    ts.forEachChild(node, visit);
  };
  visit(file);
  return result;
}

/** Trace every local value import from shipped executable entries, including lifecycle and popup collaborators. */
function reachableModules(entries: string[]): Map<string, string> {
  const files = new Map<string, string>();
  const visit = (filename: string): void => {
    if (files.has(filename)) return;
    const source = readFileSync(filename, 'utf8');
    files.set(filename, source);
    for (const specifier of runtimeSpecifiers(source)) {
      if (!specifier.startsWith('.')) continue;
      const base = path.resolve(path.dirname(filename), specifier);
      const next = [base, base + '.js', base + '.ts', path.join(base, 'index.ts')].find(
        (candidate) => existsSync(candidate) && /\.(?:[cm]?js|tsx?|json)$/.test(candidate),
      );
      assert.ok(next, `Unresolved runtime import ${filename}: ${specifier}`);
      visit(next);
    }
  };
  for (const entry of entries) visit(entry);
  return files;
}

it('all shipped native main and preload dependencies exclude internal protocol and browser-launch backends', () => {
  const entries = ['main/main.ts', 'preload/index.ts', 'preload/dialog.ts', 'preload/recording.ts'];
  const reachable = reachableModules(entries.map((entry) => path.resolve(main, '..', entry)));
  const failures = [...reachable].flatMap(([filename, source]) =>
    violations(source).map((detail) => `${path.relative(main, filename)}: ${detail}`),
  );
  assert.deepEqual(failures, []);
  for (const filename of reachable.keys())
    assert.doesNotMatch(
      filename,
      /\/(?:front-door|cdp|worker)\/|\/mirror\/(?:capture-process|cdp-ws)\.|\/recording\/frame-sessions\./,
    );
  assert.ok(reachable.has(path.join(main, 'tabs/protection.ts')), 'Gate includes page and popup lifecycle');
  assert.ok(reachable.has(path.join(main, 'tabs/workers.ts')), 'Gate includes worker lifecycle');
});

it('runtime graph includes reexports and dynamic imports but excludes type-only aliases', () => {
  assert.deepEqual(
    runtimeSpecifiers(
      'import type { X } from "./old.ts"; import { type Y } from "./old2.ts"; export type { Z } from "./old3.ts"; export { thing } from "./real.ts"; await import("./lazy.ts"); require("./cjs.cjs")',
    ),
    ['./real.ts', './lazy.ts', './cjs.cjs'],
  );
});
