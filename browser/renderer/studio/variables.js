/**
 * The studio's variables: name, secret flag and default for each, and the
 * run inputs for every {{variable}} the steps use.
 */
/* global Dom, Studio */
/* exported StudioVariables */

/** Variables and run inputs. */
const StudioVariables = {
  /** A `{{variable}}` placeholder. */
  PLACEHOLDER: /\{\{([A-Za-z_]\w*)\}\}/g,

  /** Redraws the variables and run inputs when they or the step values changed. */
  render(variables) {
    const values = Studio.state.draft.steps.map((s) => [s.text, s.url, s.expected, s.file, s.option]);
    const signature = JSON.stringify([variables, values]);
    if (signature === Studio.signatures.variables) return;
    Studio.signatures.variables = signature;
    const host = Dom.byId('workflow-variables');
    host.replaceChildren();
    for (const [name, config] of Object.entries(variables)) host.append(StudioVariables.row(variables, name, config));
    StudioVariables.inputs(variables);
  },

  /** Replaces the variables. */
  save(variables) {
    return Studio.command({ type: 'variables', variables });
  },

  /** One variable: rename, secret, default and remove. */
  row(variables, name, config) {
    const row = Dom.node('div', null, 'variable-row');
    const rename = (nextName) => Studio.command({ type: 'rename-variable', name, nextName });
    row.append(Studio.field('Variable name', name, rename));
    row.append(StudioVariables.secretToggle(variables, name, config));
    if (!config.secret) row.append(StudioVariables.defaultField(variables, name, config));
    row.append(Studio.button('Remove', () => StudioVariables.remove(variables, name)));
    return row;
  },

  /** A non-secret variable's default value. */
  defaultField(variables, name, config) {
    const setDefault = (value) => StudioVariables.save({ ...variables, [name]: { ...config, default: value } });
    return Studio.field('Default', config.default || '', setDefault);
  },

  /** The Secret checkbox; a secret loses its default. */
  secretToggle(variables, name, config) {
    const secret = Dom.node('label', null, 'studio-checkbox');
    const check = Dom.node('input');
    check.type = 'checkbox';
    check.checked = !!config.secret;
    check.addEventListener('change', () => StudioVariables.setSecret(variables, name, config, check.checked));
    secret.append(check, Dom.node('span', 'Secret'));
    return secret;
  },

  /** Marks a variable secret (dropping its default) or not. */
  setSecret(variables, name, config, secret) {
    const next = { ...config, secret, ...(secret ? { default: undefined } : {}) };
    StudioVariables.save({ ...variables, [name]: next });
  },

  /** Removes a variable. */
  remove(variables, name) {
    const next = { ...variables };
    delete next[name];
    StudioVariables.save(next);
  },

  /** One input per variable the steps use; secrets are password fields. */
  inputs(variables) {
    const used = [
      ...new Set([...JSON.stringify(Studio.state.draft.steps).matchAll(StudioVariables.PLACEHOLDER)].map((m) => m[1])),
    ];
    Dom.byId('run-inputs').replaceChildren(...used.map((name) => StudioVariables.input(variables, name)));
  },

  /** A run input for one variable, prefilled with its default. */
  input(variables, name) {
    const secret = variables[name]?.secret;
    const label = `${name}${secret ? ' · secret' : ''}`;
    const el = Studio.field(label, variables[name]?.default || '', () => {}, secret ? 'password' : 'text');
    el.querySelector('input').dataset.variable = name;
    el.querySelector('input').autocomplete = 'off';
    return el;
  },
};
