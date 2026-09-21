/**
 * The studio's variables: name, secret flag and default for each, and the
 * run inputs for every {{variable}} the steps use. Handlers read the
 * variables as they are when the event fires, so two quick edits both land;
 * the run inputs are rebuilt only when the set of names changes, so what was
 * typed into them survives.
 */
/* global Dom, Studio */
/* exported StudioVariables */

/** Variables and run inputs. */
const StudioVariables = {
  /** A `{{variable}}` placeholder. */
  PLACEHOLDER: /\{\{([A-Za-z_]\w*)\}\}/g,

  /** Redraws the variable rows when the variables changed, and the run inputs when their names did. */
  render(variables) {
    const signature = JSON.stringify(variables);
    if (signature !== Studio.signatures.variables) StudioVariables.rows(variables, signature);
    StudioVariables.inputs(variables);
  },

  /** Rebuilds the variable rows, keeping focus on the field the person was in. */
  rows(variables, signature) {
    Studio.signatures.variables = signature;
    const host = Dom.byId('workflow-variables');
    Studio.keepFocus(host, () => {
      host.replaceChildren();
      for (const [name, config] of Object.entries(variables)) host.append(StudioVariables.row(name, config));
    });
  },

  /** The variables as they are now. */
  current: () => Studio.state.draft.variables,

  /** Replaces the variables with what `next` makes of them as they are when its turn comes. */
  save(next) {
    return Studio.command(() => ({ type: 'variables', variables: next(StudioVariables.current()) }));
  },

  /** Changes one variable's settings, on top of the variables as they are then. */
  change(name, changes) {
    return StudioVariables.save((variables) => ({ ...variables, [name]: { ...variables[name], ...changes } }));
  },

  /** One variable: rename, secret, default and remove. */
  row(name, config) {
    const row = Dom.node('div', null, 'variable-row');
    const rename = (nextName) => Studio.command({ type: 'rename-variable', name, nextName });
    row.append(Studio.field('Name', name, rename, 'text', name + ':name'));
    row.append(StudioVariables.secretToggle(name, config));
    if (!config.secret) row.append(StudioVariables.defaultField(name, config));
    row.append(Studio.button('Remove', () => StudioVariables.remove(name)));
    return row;
  },

  /** A non-secret variable's default value. */
  defaultField(name, config) {
    const setDefault = (value) => StudioVariables.change(name, { default: value });
    return Studio.field('Default', config.default || '', setDefault, 'text', name + ':default');
  },

  /** The Secret checkbox; a secret loses its default. */
  secretToggle(name, config) {
    const secret = Dom.node('label', null, 'studio-checkbox');
    const check = Dom.node('input');
    Object.assign(check, { type: 'checkbox', checked: !!config.secret });
    check.dataset.key = name + ':secret';
    check.addEventListener('change', () => StudioVariables.setSecret(name, check.checked));
    secret.append(check, Dom.node('span', 'Secret'));
    return secret;
  },

  /** Marks a variable secret (dropping its default) or not. */
  setSecret(name, secret) {
    StudioVariables.change(name, { secret, ...(secret ? { default: undefined } : {}) });
  },

  /** Removes a variable. */
  remove(name) {
    StudioVariables.save((variables) => {
      const next = { ...variables };
      delete next[name];
      return next;
    });
  },

  /** One input per variable the steps use, rebuilt only when the names or secret flags change. */
  inputs(variables) {
    const used = [
      ...new Set([...JSON.stringify(Studio.state.draft.steps).matchAll(StudioVariables.PLACEHOLDER)].map((m) => m[1])),
    ];
    const signature = JSON.stringify(used.map((name) => [name, !!variables[name]?.secret]));
    if (signature === Studio.signatures.inputs) return;
    Studio.signatures.inputs = signature;
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
