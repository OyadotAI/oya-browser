/**
 * The welcome panel's account chooser: the accounts this browser signed out of,
 * one row each, as a mail app lists them. A click signs straight back in; the
 * last row starts a fresh sign-in. Styles are in account-chooser.css.
 */
import type { CSSProperties } from 'react';
import type { SavedAccountView } from '../../../../shared/ipc.ts';
import { useViewModel } from '../../../hooks/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import './account-chooser.css';

/** One account row's props. */
interface RowProps extends ViewProps<SetupViewModel> {
  /** The account it signs back in to. */
  account: SavedAccountView;
  /** Its place in the list, for the staggered entrance. */
  index: number;
}

/** The row's entrance delay, read by the stylesheet. */
const stagger = (index: number): CSSProperties => ({ '--row': index }) as CSSProperties;

/** Who, project and server: the email under the name, then the project and the host in small type. */
function AccountText({ account }: Pick<RowProps, 'account'>) {
  return (
    <span className="chooser-text">
      <strong>{account.label}</strong>
      {account.email && <span className="chooser-email">{account.email}</span>}
      <span className="chooser-meta">
        {account.project && <span className="chooser-project">{account.project}</span>}
        {account.host && <span className="chooser-host">{account.host}</span>}
      </span>
    </span>
  );
}

/** One remembered account: the whole row signs in; the cross, shown on hover or focus, forgets it. */
function AccountRow({ vm, account, index }: RowProps) {
  const { busy, choosing } = useViewModel(vm);
  const opening = choosing === account.id;
  return (
    <li className="chooser-item" style={stagger(index)} data-opening={opening || undefined}>
      <button className="chooser-row" disabled={busy} onClick={() => void vm.useAccount(account.id)}>
        <span className="chooser-avatar" aria-hidden="true">
          {account.label.slice(0, 1).toUpperCase()}
        </span>
        <AccountText account={account} />
        <span className="chooser-go" aria-hidden="true">
          {opening ? <span className="chooser-spinner" /> : '→'}
        </span>
      </button>
      <button
        className="chooser-remove"
        disabled={busy}
        aria-label={`Remove ${account.label} from this browser`}
        title="Remove from this browser"
        onClick={() => void vm.forgetAccount(account.id)}
      >
        ×
      </button>
    </li>
  );
}

/** The last row: sign in to an account that is not listed. */
function OtherAccountRow({ vm, index }: Pick<RowProps, 'vm' | 'index'>) {
  const { busy } = useViewModel(vm);
  return (
    <li className="chooser-item" style={stagger(index)}>
      <button className="chooser-row chooser-other" disabled={busy} onClick={() => vm.show('checklist')}>
        <span className="chooser-avatar" aria-hidden="true">
          +
        </span>
        <span className="chooser-text">
          <strong>Use another account</strong>
        </span>
      </button>
    </li>
  );
}

/** Accounts: the chooser the welcome screen opens on while any are remembered. */
export function AccountChooser({ vm }: ViewProps<SetupViewModel>) {
  const { accounts } = useViewModel(vm);
  return (
    <div className="welcome-view" data-view="accounts">
      <h2 tabIndex={-1}>Welcome back</h2>
      <p className="sub">Choose an account to pick up where you left off.</p>
      <ul className="chooser" aria-label="Accounts on this browser">
        {accounts.map((account, index) => (
          <AccountRow key={account.id} vm={vm} account={account} index={index} />
        ))}
        <OtherAccountRow vm={vm} index={accounts.length} />
      </ul>
    </div>
  );
}
