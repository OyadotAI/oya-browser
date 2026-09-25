/**
 * The model picker in Ask's model card: a button naming the chosen model that
 * opens a searchable list. Every word typed must match a model's name or id;
 * OpenRouter's models are grouped by vendor; the arrow keys, Enter and Escape
 * work as in any list; and an id the list lacks can be used as typed. Model
 * names come from the server, and OpenRouter's are third-party text, so they
 * are only ever set as text, never as markup.
 */
/* global Dom, ShellIcons */
/* exported ModelPicker */

/** "Vendor: Model" (OpenRouter's naming) as its vendor and model name; other names have no vendor. */
function splitModelLabel(label) {
  const [group, ...rest] = label.split(': ');
  return rest.length && group ? { group, name: rest.join(': ') } : { group: '', name: label };
}

/** The words of a search, lowercased. */
const searchWords = (query) => query.toLowerCase().split(/\s+/).filter(Boolean);

/** `text` with every searched word wrapped in <mark>, built as nodes. */
function markMatches(text, words) {
  if (!words.length) return [document.createTextNode(text)];
  const escaped = words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  // Splitting on a captured pattern alternates: text, match, text, match, …
  let matched = true;
  return text.split(new RegExp(`(${escaped.join('|')})`, 'i')).map((part) => {
    matched = !matched;
    return matched ? Dom.node('mark', part) : document.createTextNode(part);
  });
}

/** The models matching `query`, then the query itself as a custom id when no model has exactly that id. */
function matchingModels(models, query) {
  const words = searchWords(query);
  const hits = models.filter((m) => words.every((w) => `${m.label} ${m.id}`.toLowerCase().includes(w)));
  const custom = query && !models.some((m) => m.id === query) ? [{ id: query, label: query, custom: true }] : [];
  return [...hits, ...custom];
}

/** What the picker's keys do while its list is open. */
const PICKER_KEYS = {
  ArrowDown: () => ModelPicker.move(1),
  ArrowUp: () => ModelPicker.move(-1),
  Enter: () => ModelPicker.chooseActive(),
  Escape: () => ModelPicker.close(true),
};

/** The searchable model picker. */
const ModelPicker = {
  /** Every model the provider offers. */
  models: [],
  /** The chosen model id. */
  value: '',
  /** The options listed now, in order. */
  shown: [],
  /** The index of the option the keyboard is on. */
  active: 0,

  /** Offers `models`, with `value` chosen, and closes the list. */
  set(models, value) {
    Object.assign(ModelPicker, { models, value });
    ModelPicker.close(false);
    ModelPicker.renderTrigger();
  },

  /** The chosen model, or an entry for an id the list does not have. */
  chosen() {
    const { models, value } = ModelPicker;
    return models.find((m) => m.id === value) || { id: value, label: value || 'Choose a model', custom: !!value };
  },

  /** The button: the chosen model's name, and its id underneath when that says more. */
  renderTrigger() {
    const model = ModelPicker.chosen();
    const { group, name } = splitModelLabel(model.label);
    Dom.byId('chat-model-name').textContent = name;
    const detail = model.custom ? 'Custom model id' : model.id !== name ? model.id : group;
    Dom.byId('chat-model-id').textContent = detail;
  },

  /** Whether the list is open. */
  isOpen() {
    return !Dom.byId('chat-model-menu').hidden;
  },

  /** Opens the list with an empty search, the chosen model in view. */
  open() {
    Dom.byId('chat-model-menu').hidden = false;
    Dom.byId('chat-model-model').setAttribute('aria-expanded', 'true');
    Dom.byId('chat-model-search').value = '';
    ModelPicker.filter();
    Dom.byId('chat-model-search').focus();
  },

  /** Closes the list; `refocus` puts focus back on the button, as Escape and a choice do. */
  close(refocus) {
    Dom.byId('chat-model-menu').hidden = true;
    Dom.byId('chat-model-model').setAttribute('aria-expanded', 'false');
    if (refocus) Dom.byId('chat-model-model').focus();
  },

  /** The button was clicked. */
  toggle() {
    if (ModelPicker.isOpen()) ModelPicker.close(true);
    else ModelPicker.open();
  },

  /** Lists the models matching the search, on the chosen one when it is listed. */
  filter() {
    const query = Dom.byId('chat-model-search').value.trim();
    ModelPicker.shown = matchingModels(ModelPicker.models, query);
    const chosen = ModelPicker.shown.findIndex((o) => o.id === ModelPicker.value);
    ModelPicker.active = query || chosen < 0 ? 0 : chosen;
    ModelPicker.renderList(searchWords(query));
  },

  /** Draws the options, grouped by vendor when there is more than one, and the count. */
  renderList(words) {
    const { shown } = ModelPicker;
    const grouped = new Set(shown.filter((o) => !o.custom).map((o) => splitModelLabel(o.label).group)).size > 1;
    const rows = shown.flatMap((o, i) => [...ModelPicker.heading(o, i, grouped), ModelPicker.row(o, i, words)]);
    Dom.byId('chat-model-list').replaceChildren(...(rows.length ? rows : [ModelPicker.empty()]));
    const models = shown.filter((o) => !o.custom).length;
    Dom.byId('chat-model-count').textContent = `${models} ${models === 1 ? 'model' : 'models'}`;
    ModelPicker.highlight();
  },

  /** A vendor heading above the first of its models, when the list is grouped. */
  heading(option, index, grouped) {
    const group = splitModelLabel(option.label).group;
    const previous = index ? splitModelLabel(ModelPicker.shown[index - 1].label).group : null;
    if (!grouped || option.custom || !group || group === previous) return [];
    return [Dom.node('li', group, 'model-group')];
  },

  /** One option: its name with the search marked, its id, and a tick when chosen. */
  row(option, index, words) {
    const li = Dom.node('li', null, option.custom ? 'model-option model-option-custom' : 'model-option');
    li.id = `chat-model-option-${index}`;
    Object.assign(li.dataset, { index: String(index), id: option.id });
    li.setAttribute('role', 'option');
    li.setAttribute('aria-selected', String(option.id === ModelPicker.value));
    li.append(ModelPicker.optionText(option, words), ModelPicker.tick());
    return li;
  },

  /** The tick a chosen option shows: the app's own icon, never server text. */
  tick() {
    const tick = Dom.node('span', null, 'model-option-check');
    tick.innerHTML = ShellIcons.icon('check');
    return tick;
  },

  /** An option's two lines: the name (or the custom id) and what sits under it. */
  optionText(option, words) {
    const text = Dom.node('span', null, 'model-option-text');
    const name = Dom.node('span', null, 'model-option-name');
    // The custom row is the query itself: nothing in it to mark.
    const shown = option.custom ? [document.createTextNode(`Use “${option.id}”`)] : null;
    name.append(...(shown || markMatches(splitModelLabel(option.label).name, words)));
    const detail = Dom.node('span', option.custom ? 'as a custom model id' : option.id, 'model-option-id');
    text.append(name, detail);
    return text;
  },

  /** What an empty provider list says. */
  empty() {
    return Dom.node('li', 'Type a model id to use it.', 'model-empty');
  },

  /** Marks the option the keyboard is on and keeps it in view. */
  highlight() {
    const options = Dom.byId('chat-model-list').querySelectorAll('[role="option"]');
    options.forEach((o) => o.classList.toggle('is-active', Number(o.dataset.index) === ModelPicker.active));
    const active = Dom.byId(`chat-model-option-${ModelPicker.active}`);
    Dom.byId('chat-model-search').setAttribute('aria-activedescendant', active ? active.id : '');
    active?.scrollIntoView({ block: 'nearest' });
  },

  /** Moves the keyboard `delta` options, wrapping at either end. */
  move(delta) {
    const count = ModelPicker.shown.length;
    if (!count) return;
    ModelPicker.active = (ModelPicker.active + delta + count) % count;
    ModelPicker.highlight();
  },

  /** Chooses `id` and closes the list. */
  choose(id) {
    ModelPicker.value = id;
    ModelPicker.renderTrigger();
    ModelPicker.close(true);
  },

  /** Enter: chooses the option the keyboard is on. */
  chooseActive() {
    const option = ModelPicker.shown[ModelPicker.active];
    if (option) ModelPicker.choose(option.id);
  },

  /** A key in the search box: moves, chooses or closes; anything else types. */
  onKey(e) {
    if (!Object.hasOwn(PICKER_KEYS, e.key)) return;
    e.preventDefault();
    e.stopPropagation();
    PICKER_KEYS[e.key]();
  },

  /** A click in the list chooses the option under it. */
  onClick(e) {
    const option = e.target.closest('[role="option"]');
    if (option) ModelPicker.choose(ModelPicker.shown[Number(option.dataset.index)].id);
  },

  /** The pointer over an option makes it the one Enter chooses. */
  onHover(e) {
    const option = e.target.closest('[role="option"]');
    if (!option || Number(option.dataset.index) === ModelPicker.active) return;
    ModelPicker.active = Number(option.dataset.index);
    ModelPicker.highlight();
  },

  /** A press outside the picker closes the list. */
  onOutside(e) {
    if (ModelPicker.isOpen() && !e.target.closest?.('#chat-model-picker')) ModelPicker.close(false);
  },
};

Dom.byId('chat-model-model').addEventListener('click', ModelPicker.toggle);
Dom.byId('chat-model-model').addEventListener('keydown', (e) => {
  if (e.key === 'ArrowDown' && !ModelPicker.isOpen()) (e.preventDefault(), ModelPicker.open());
});
Dom.byId('chat-model-search').addEventListener('input', ModelPicker.filter);
Dom.byId('chat-model-search').addEventListener('keydown', ModelPicker.onKey);
Dom.byId('chat-model-list').addEventListener('click', ModelPicker.onClick);
Dom.byId('chat-model-list').addEventListener('mousemove', ModelPicker.onHover);
document.addEventListener('mousedown', ModelPicker.onOutside);
