/** Translation consent uses the shell's existing dialog, focus trap and native-view-safe overlay. */
import { createPortal } from 'react-dom';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Dialog } from '../../../ui/index.ts';
import { confirmationText, type TranslateDialogViewModel } from '../view-models/translate-dialog-view-model.ts';
import './translate-dialog.css';

/** Main-owned confirmation rendered only after the native page is behind the shell. */
export function TranslateDialog({
  vm,
}: {
  /** The confirmation and its explicit actions. */ vm: TranslateDialogViewModel;
}) {
  const { confirmation } = useViewModel(vm);
  const copy = confirmation && confirmationText(confirmation);
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
          </div>
          <footer>
            <Button variant="secondary" id="translate-cancel" onClick={() => vm.cancel()}>
              Cancel
            </Button>
            <Button variant="primary" id="translate-confirm" onClick={() => vm.confirm()}>
              Translate
            </Button>
          </footer>
        </div>
      )}
    </Dialog>,
    document.body,
  );
}
