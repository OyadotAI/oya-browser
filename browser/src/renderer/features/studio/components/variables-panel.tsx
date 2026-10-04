/**
 * The Variables section of the Steps tab: name, secret flag and default for
 * each variable, Remove, and Add variable. Everything is off while the draft
 * is locked.
 */
import { isDisabled, studioMode } from '../model/studio-model.ts';
import { StudioField } from './fields.tsx';
import type { StudioPartProps } from './record-pane.tsx';
import type { StudioViewModel } from '../view-models/studio-view-model.ts';
import type { VariableConfig } from '../model/types.ts';
import { Button } from '../../../ui/index.ts';

/** The variables and Add variable. */
export function VariablesPanel({ vm, state }: StudioPartProps) {
  const locked = studioMode(state).locked;
  const variables = Object.entries(state.snapshot?.draft.variables ?? {});
  return (
    <details className="studio-details" id="variables-details">
      <summary>Variables</summary>
      <div id="workflow-variables">
        {variables.map(([name, config]) => (
          <VariableRow key={name} vm={vm} name={name} config={config} locked={locked} />
        ))}
      </div>
      <Button
        id="variable-add"
        disabled={isDisabled(state, 'variable-add')}
        onClick={() => void vm.actions.addVariable()}
      >
        + Add variable
      </Button>
      <p className="studio-hint">
        Use {'{{variable_name}}'} in a step. Secret values are asked for on each run and never saved.
      </p>
    </details>
  );
}

/** What a variable row is drawn from. */
export interface VariableRowProps {
  /** The studio. */
  vm: StudioViewModel;
  /** The variable's name. */
  name: string;
  /** Its settings. */
  config: VariableConfig;
  /** Every control is off. */
  locked: boolean;
}

/** One variable: rename, secret, default (not for a secret) and remove. */
export function VariableRow({ vm, name, config, locked }: VariableRowProps) {
  const vars = vm.variables;
  return (
    <div className="variable-row">
      <StudioField
        label="Name"
        value={name}
        fieldKey={name + ':name'}
        disabled={locked}
        onCommit={(next) => void vars.rename(name, String(next ?? ''))}
      />
      <label className="studio-checkbox">
        <input
          type="checkbox"
          data-key={name + ':secret'}
          checked={!!config.secret}
          disabled={locked}
          onChange={(event) => void vars.setSecret(name, event.target.checked)}
        />
        <span>Secret</span>
      </label>
      {!config.secret && (
        <StudioField
          label="Default"
          value={config.default || ''}
          fieldKey={name + ':default'}
          disabled={locked}
          onCommit={(value) => void vars.setDefault(name, String(value ?? ''))}
        />
      )}
      <Button type="button" title="Remove" disabled={locked} onClick={() => void vars.remove(name)}>
        Remove
      </Button>
    </div>
  );
}
