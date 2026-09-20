/**
 * The project's shared lint rules, used by every part (server, browser, ui,
 * packages) so the whole codebase is held to one standard. This file has no
 * dependencies: each part brings its own ESLint and plugins and imports the
 * rule sets from here. See ARCHITECTURE.md for the reasoning behind them.
 */

/** A file opens with a `/** … *\/` comment saying what it is for (after any shebang). */
const fileHeader = {
  meta: { type: 'suggestion', messages: { missing: 'Start the file with a /** … */ comment describing it.' } },
  create: (context) => ({
    Program(node) {
      const first = context.sourceCode.getAllComments().find((c) => c.type !== 'Shebang');
      const code = node.body[0];
      const isDoc = first?.type === 'Block' && first.value.startsWith('*');
      if (!isDoc || (code && first.range[0] > code.range[0])) context.report({ node, messageId: 'missing' });
    },
  }),
};

/** A class stays within a line budget; past it, it is doing more than one job. */
const maxClassLines = {
  meta: {
    type: 'suggestion',
    schema: [{ type: 'integer' }],
    messages: { long: 'Class is {{lines}} lines (max {{max}}): split its responsibilities.' },
  },
  create: (context) => ({
    'ClassDeclaration, ClassExpression'(node) {
      const max = context.options[0] ?? MAX_CLASS_LINES;
      const lines = node.loc.end.line - node.loc.start.line + 1;
      if (lines > max) context.report({ node, messageId: 'long', data: { lines, max } });
    },
  }),
};

/** Longest function, in lines of code (blank lines and comments excluded). */
export const MAX_FUNCTION_LINES = 10;
/** Longest React component: JSX is layout, not logic, so it gets more room. Logic still lives in hooks. */
export const MAX_COMPONENT_LINES = 50;
/** Longest class. */
export const MAX_CLASS_LINES = 200;

/** Register as `plugins: { local: localPlugin }`. */
export const localPlugin = { rules: { 'file-header': fileHeader, 'max-class-lines': maxClassLines } };

/** House conventions every part shares. */
export const baseRules = {
  'local/file-header': 'error',
  // `catch {}` is how this codebase says "best effort, ignore".
  'no-empty': ['error', { allowEmptyCatch: true }],
  // Polling loops are `do { … } while (true)` with their exit inside.
  'no-constant-condition': ['error', { checkLoops: false }],
};

/** Short functions, short classes, no magic numbers. `maxLines` is raised only for React components. */
export const designRules = (maxLines = MAX_FUNCTION_LINES) => ({
  'max-lines-per-function': ['error', { max: maxLines, skipBlankLines: true, skipComments: true }],
  'local/max-class-lines': ['error', MAX_CLASS_LINES],
  'no-magic-numbers': [
    'error',
    { ignore: [0, 1, -1], ignoreArrayIndexes: true, ignoreDefaultValues: true, ignoreClassFieldInitialValues: true },
  ],
});

/** What must carry a doc comment: every function, class, method, field, type and member. */
const DOC_CONTEXTS = [
  'PropertyDefinition',
  'TSInterfaceDeclaration',
  'TSTypeAliasDeclaration',
  'TSPropertySignature',
  'Program > ExportNamedDeclaration:has(> VariableDeclaration)',
  'Program > VariableDeclaration:has(> VariableDeclarator > :matches(ArrowFunctionExpression, FunctionExpression))',
];

/** Every route registration says what it does. */
export const ROUTE_DOC_CONTEXTS = [
  'Program > ExpressionStatement > CallExpression[callee.object.name=/[rR]outer$/][callee.property.name=/^(get|post|put|patch|delete|use)$/]',
  'BlockStatement > ExpressionStatement > CallExpression[callee.object.name=/[rR]outer$/][callee.property.name=/^(get|post|put|patch|delete)$/]',
];

/** Doc-comment requirements (register eslint-plugin-jsdoc as `jsdoc`); `extra` adds contexts. */
export const docRules = (extra = []) => ({
  'jsdoc/require-jsdoc': [
    'error',
    {
      require: { FunctionDeclaration: true, ClassDeclaration: true, MethodDefinition: true },
      contexts: [...DOC_CONTEXTS, ...extra],
      checkConstructors: false,
    },
  ],
});
