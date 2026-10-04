/**
 * The connection settings dialog over the page (index.html's
 * #reconnect-overlay): server, key and name, Save & Reconnect, and Cancel.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { Dialog, TextField } from '../../../ui/index.ts';
import type { ReconnectField, ReconnectViewModel } from '../view-models/reconnect-view-model.ts';
import type { ViewProps } from '../model/models.ts';

/** One labelled field of the form. */
interface FieldProps extends ViewProps<ReconnectViewModel> {
  /** The state field it edits. */
  field: ReconnectField;
  /** The input's id. */
  id: string;
  /** Its label. */
  label: string;
  /** Its input type. */
  type: 'text' | 'password';
  /** Its placeholder. */
  placeholder: string;
}

/** A labelled field. */
function Field({ vm, field, id, label, type, placeholder }: FieldProps) {
  const value = useViewModel(vm)[field];
  return (
    <TextField
      id={id}
      label={label}
      type={type}
      placeholder={placeholder}
      spellCheck={type === 'text' ? false : undefined}
      value={value}
      onChange={(event) => vm.edit(field, event.target.value)}
    />
  );
}

/** The reconnect dialog; a click on its backdrop closes it. */
export function ReconnectOverlay({ vm }: ViewProps<ReconnectViewModel>) {
  const s = useViewModel(vm);
  return (
    <Dialog
      className={s.open ? 'reconnect-overlay open' : 'reconnect-overlay'}
      id="reconnect-overlay"
      label="Connection settings"
      open={s.open}
      onClose={() => vm.close()}
      focusId="reconn-server"
    >
      <div className="setup-card">
        <div className="logo">Server Connection</div>
        <div className="sub" id="reconn-sub">
          Update connection settings
        </div>
        <Field
          vm={vm}
          field="server"
          id="reconn-server"
          label="Server address"
          type="text"
          placeholder="wss://oyabrowser.com/ws"
        />
        <Field vm={vm} field="apiKey" id="reconn-key" label="API key" type="password" placeholder="API key" />
        <Field vm={vm} field="name" id="reconn-name" label="Browser name" type="text" placeholder="Oya Browser" />
        <div role="alert" className="setup-error" id="reconn-error">
          {s.error}
        </div>
        <div className="setup-btns">
          <button className="btn-skip" id="reconn-cancel" onClick={() => vm.close()}>
            Cancel
          </button>
          <button className="btn-connect" id="reconn-save" disabled={s.saving} onClick={() => void vm.save()}>
            {s.button}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
