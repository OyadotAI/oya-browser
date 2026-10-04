/**
 * The model picker (`#chat-model-picker`): the button naming the chosen model
 * and the searchable list under it. Focus moves to the search when the list
 * opens and back to the button on a choice or Escape; a press outside closes
 * it. Model names are set as text, never as markup.
 */
import { useCallback, useEffect, useRef, type KeyboardEvent } from 'react';
import { useClickOutside, useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import { ASK_TEXT } from '../model/constants.ts';
import type { ModelOption } from '../model/types.ts';
import type { ModelPickerViewModel } from '../view-models/model-picker-view-model.ts';
import {
  groupHeading,
  isGrouped,
  markParts,
  modelCount,
  searchWords,
  splitModelLabel,
  triggerText,
} from '../model/model-search.ts';

/** What the picker is given. */
interface PickerProps {
  /** The picker. */
  picker: ModelPickerViewModel;
}

/** What the list's parts are given. */
interface ListProps extends PickerProps {
  /** Puts focus back on the button after a choice. */
  onChosen: () => void;
}

/** What one option is given. */
interface OptionProps extends ListProps {
  /** The option. */
  option: ModelOption;
  /** Its place in the list. */
  index: number;
  /** The searched words, to mark. */
  words: string[];
}

/** The id of option `index`. */
const optionId = (index: number): string => `chat-model-option-${index}`;

/** One option: its name with the search marked (or "Use “id”" for a typed id), its id, and the tick. */
function OptionRow({ picker, option, index, words, onChosen }: OptionProps) {
  const { value, active } = useViewModel(picker);
  const name = option.custom
    ? [{ text: `Use “${option.id}”`, mark: false }]
    : markParts(splitModelLabel(option.label).name, words);
  return (
    <li
      id={optionId(index)}
      className={`${option.custom ? 'model-option model-option-custom' : 'model-option'}${index === active ? ' is-active' : ''}`}
      data-index={index}
      data-id={option.id}
      role="option"
      aria-selected={option.id === value}
      onClick={() => (picker.choose(option.id), onChosen())}
      onMouseMove={() => index !== active && picker.hover(index)}
    >
      <span className="model-option-text">
        <span className="model-option-name">
          {name.map((p, i) => (p.mark ? <mark key={i}>{p.text}</mark> : p.text))}
        </span>
        <span className="model-option-id">{option.custom ? ASK_TEXT.asCustom : option.id}</span>
      </span>
      <span className="model-option-check">
        <Icon name="check" />
      </span>
    </li>
  );
}

/** The list's rows: a vendor heading above the first of its models when grouped, then each option. */
function OptionRows({ picker, onChosen }: ListProps) {
  const { shown, query } = useViewModel(picker);
  const words = searchWords(query.trim());
  const grouped = isGrouped(shown);
  if (!shown.length) return <li className="model-empty">{ASK_TEXT.typeModel}</li>;
  return shown.map((option, i) => [
    groupHeading(shown, i, grouped) && (
      <li key={`g${i}`} className="model-group">
        {groupHeading(shown, i, grouped)}
      </li>
    ),
    <OptionRow key={i} picker={picker} option={option} index={i} words={words} onChosen={onChosen} />,
  ]);
}

/** The open list: the search, the count and the options; focuses the search and keeps the active option in view. */
function ModelMenu({ picker, onChosen }: ListProps) {
  const { open, query, shown, active } = useViewModel(picker);
  const search = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => void (open && search.current?.focus()), [open]);
  useEffect(
    () => void list.current?.querySelector(`#${optionId(active)}`)?.scrollIntoView({ block: 'nearest' }),
    [active, shown],
  );
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!picker.key(e.key)) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Enter' || e.key === 'Escape') onChosen();
  };
  return (
    <div className="model-menu" id="chat-model-menu" hidden={!open}>
      <div className="model-search">
        <span className="model-search-icon" data-icon="search">
          <Icon name="search" />
        </span>
        <input
          type="text"
          id="chat-model-search"
          ref={search}
          role="combobox"
          aria-label="Search models"
          aria-expanded="true"
          aria-controls="chat-model-list"
          aria-autocomplete="list"
          aria-activedescendant={shown.length ? optionId(active) : ''}
          autoComplete="off"
          spellCheck={false}
          placeholder="Search or type a model id"
          value={query}
          onChange={(e) => picker.search(e.target.value)}
          onKeyDown={onKey}
        />
        <span className="model-count" id="chat-model-count">
          {modelCount(shown)}
        </span>
      </div>
      <ul className="model-list" id="chat-model-list" role="listbox" aria-label="Models" ref={list}>
        <OptionRows picker={picker} onChosen={onChosen} />
      </ul>
    </div>
  );
}

/** The model picker. */
export function ModelPicker({ picker }: PickerProps) {
  const { models, value, open } = useViewModel(picker);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const inside = useCallback((target: Node) => !!root.current?.contains(target), []);
  useClickOutside(
    open,
    inside,
    useCallback(() => picker.close(), [picker]),
  );
  const { name, detail } = triggerText(models, value);
  const arrowOpens = (e: KeyboardEvent) => e.key === 'ArrowDown' && !open && (e.preventDefault(), picker.openList());
  return (
    <div className="model-picker" id="chat-model-picker" ref={root}>
      <button
        type="button"
        className="model-trigger"
        id="chat-model-model"
        ref={trigger}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-labelledby="chat-model-model-label chat-model-name"
        onClick={() => picker.toggle()}
        onKeyDown={arrowOpens}
      >
        <span className="model-trigger-text">
          <span className="model-trigger-name" id="chat-model-name">
            {name}
          </span>
          <span className="model-trigger-id" id="chat-model-id">
            {detail}
          </span>
        </span>
        <span className="model-trigger-chevron" data-icon="chevron">
          <Icon name="chevron" />
        </span>
      </button>
      <ModelMenu picker={picker} onChosen={() => trigger.current?.focus()} />
    </div>
  );
}
