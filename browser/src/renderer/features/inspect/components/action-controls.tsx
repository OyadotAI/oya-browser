/**
 * The Actions pane's controls: an action button (off while another runs, or
 * while the agent holds the page), a field that runs its action on Enter, the
 * elements an analysis found, and the result box.
 */
import type { ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Icon } from '../../../ui/index.ts';
import { ACTION_ICONS, ACTIONS_TEXT, type ActionField } from '../model/constants.ts';
import { actionLook, type ActionsViewModel } from '../view-models/actions-view-model.ts';

/** What an action button is given. */
export interface ActionButtonProps {
  /** The pane. */
  vm: ActionsViewModel;
  /** The action it runs. */
  action: string;
  /** Its words. */
  children: ReactNode;
  /** A small button in a row, without an icon. */
  small?: boolean;
}

/** A button that runs `action`; a large one shows the action's icon. */
export function ActionButton({ vm, action, children, small }: ActionButtonProps) {
  const look = actionLook(useViewModel(vm), action);
  return (
    <button
      className={small ? 'action-btn action-btn-sm' : 'action-btn'}
      data-action={action}
      disabled={look.disabled}
      title={look.title}
      onClick={() => void vm.run(action)}
    >
      {!small && (
        <span className="action-icon">
          <Icon name={ACTION_ICONS[action] ?? 'code'} />
        </span>
      )}
      {children}
    </button>
  );
}

/** What a field is given. */
export interface ActionInputProps {
  /** The pane. */
  vm: ActionsViewModel;
  /** The field's id. */
  id: ActionField;
  /** Its name for assistive technology. */
  label: string;
  /** What it says while empty. */
  placeholder: string;
  /** A narrow number field. */
  number?: boolean;
}

/** A field; Enter runs its action. */
export function ActionInput({ vm, id, label, placeholder, number }: ActionInputProps) {
  const value = useViewModel(vm).fields[id];
  return (
    <input
      className={number ? 'action-field action-field-sm' : 'action-field'}
      id={id}
      type={number ? 'number' : 'text'}
      placeholder={placeholder}
      aria-label={label}
      spellCheck={id === 'action-url' ? false : undefined}
      value={value}
      onChange={(e) => vm.setField(id, e.target.value)}
      onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), void vm.enter(id))}
    />
  );
}

/** The analyzed page's elements; picking one fills in its number and focuses the field. */
export function ActionElements({ vm }: { /** The pane. */ vm: ActionsViewModel }) {
  const { elements, hint } = useViewModel(vm);
  const pick = (id: string) => (vm.pickElement(id), document.getElementById('action-click-id')?.focus());
  return (
    <>
      <p className="action-hint" id="action-elements-hint">
        {hint}
      </p>
      <div className="action-elements" id="action-elements" aria-label="Elements on the page">
        {elements.map((e) => (
          <button key={e.id} type="button" className="action-element" onClick={() => pick(e.id)}>
            {e.label}
          </button>
        ))}
      </div>
    </>
  );
}

/** The result box: heading, Copy, and a screenshot or text (with the cut note). */
export function ActionResultBox({ vm }: { /** The pane. */ vm: ActionsViewModel }) {
  const { result, copyLabel } = useViewModel(vm);
  const cls = ['action-result', result && 'visible', result?.failed && 'error'].filter(Boolean).join(' ');
  return (
    <section className={cls} id="action-result" aria-live="polite">
      <div className="action-result-head">
        <span id="action-result-title">{result?.title}</span>
        <Button id="action-result-copy" type="button" onClick={() => void vm.copy()}>
          {copyLabel}
        </Button>
      </div>
      <div className="action-result-body" id="action-result-body">
        {result?.image ? <img className="action-shot" src={result.image} alt={ACTIONS_TEXT.shotAlt} /> : result?.text}
        {result?.cut && <p className="action-cut">{ACTIONS_TEXT.cut}</p>}
      </div>
    </section>
  );
}
