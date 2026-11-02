/** Translation consent uses the shell's existing dialog, focus trap and native-view-safe overlay. */
import { createPortal } from 'react-dom';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Dialog, StatusLine } from '../../../ui/index.ts';
import {
  confirmationActions,
  confirmationText,
  type TranslateDialogViewModel,
} from '../view-models/translate-dialog-view-model.ts';
import './translate-dialog.css';

/** Main-owned confirmation rendered only after the native page is behind the shell. */
export function TranslateDialog({
  vm,
}: {
  /** The confirmation and its explicit actions. */ vm: TranslateDialogViewModel;
}) {
  const { confirmation, action } = useViewModel(vm);
  const copy = confirmation && confirmationText(confirmation);
  const actions = confirmationActions(action);
  return createPortal(
    <Dialog
      open={!!copy}
      hidden={!copy}
      onClose={() => vm.cancel()}
      focusId="translate-cancel"
      fallbackFocusId="url-bar"
      id="translate-overlay"
      className="shell-overlay"
      label={copy?.title || 'Translate page'}
    >
      {copy && (
        <div className="shell-dialog translation-dialog">
          <header>
            <h2>{copy.title}</h2>
          </header>
          <div className="translation-dialog-body">
            <p>{copy.disclosure}</p>
            <p>{copy.continuation}</p>
            <p className="translation-allowance">{copy.cancellation}</p>
            <StatusLine
              live
              aria-atomic="true"
              id="translate-action-status"
              className={actions.failed ? 'translation-action-error' : 'translation-allowance'}
            >
              {actions.status}
            </StatusLine>
          </div>
          <TranslationActions vm={vm} actions={actions} />
        </div>
      )}
    </Dialog>,
    document.body,
  );
}

/** Guarded buttons keep their focus target during a request or an explicit retry. */
function TranslationActions({
  vm,
  actions,
}: {
  /** Trusted proposal actions; main still revalidates every request. */ vm: TranslateDialogViewModel;
  /** Copy and disabled states derived only from this proposal's action. */ actions: ReturnType<
    typeof confirmationActions
  >;
}) {
  return (
    <footer>
      <Button
        variant="secondary"
        id="translate-cancel"
        aria-disabled={actions.cancelBlocked || undefined}
        aria-describedby={actions.failed ? 'translate-action-status' : undefined}
        onClick={() => vm.cancel()}
      >
        {actions.cancelLabel}
      </Button>
      <Button
        variant="primary"
        id="translate-confirm"
        aria-disabled={actions.confirmBlocked || undefined}
        aria-describedby={actions.failed ? 'translate-action-status' : undefined}
        onClick={() => vm.confirm()}
      >
        {actions.confirmLabel}
      </Button>
    </footer>
  );
}
