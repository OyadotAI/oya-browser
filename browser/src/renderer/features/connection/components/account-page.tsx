/**
 * The account page of the shell dialog (index.html's #profile-section): the
 * account card, Sync, Imported logins, Browsing as, This device, the server
 * and browser id under Advanced, and Switch account and Log out.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { TEXT } from '../model/constants.ts';
import type { AccountModels } from './shell-dialog.tsx';
import { AccountCardView, DeviceGroup, PersonaGroup, SyncGroup } from './account-groups.tsx';
import { ImportGroup } from './import-group.tsx';
import { accountCard } from '../view-models/account-view-model.ts';
import { Button, StatusLine } from '../../../ui/index.ts';
import './account-page.css';

/** The server and browser id with their Copy buttons, and Connection settings…; folded until asked for. */
function Advanced({ dialog, reconnect }: Pick<AccountModels, 'dialog' | 'reconnect'>) {
  const s = useViewModel(dialog);
  return (
    <details
      className="profile-advanced"
      id="profile-advanced"
      open={s.advanced}
      onToggle={(e) => dialog.setAdvanced(e.currentTarget.open)}
    >
      <summary>Advanced</summary>
      <dl>
        <dt>Server</dt>
        <dd>
          <code id="profile-server">{s.server || TEXT.noServer}</code>
          <Button id="copy-server" aria-label="Copy server address" onClick={() => void dialog.copy('server address')}>
            Copy
          </Button>
        </dd>
        <dt>Browser ID</dt>
        <dd>
          <code id="profile-browser-id">{s.browserId || TEXT.noBrowserId}</code>
          <Button
            id="copy-browser-id"
            aria-label="Copy browser ID"
            disabled={!s.browserId}
            onClick={() => void dialog.copy('browser ID')}
          >
            Copy
          </Button>
        </dd>
      </dl>
      <Button variant="secondary" id="connection-edit" onClick={() => void reconnect.editSettings()}>
        Connection settings…
      </Button>
      <StatusLine className="quiet profile-note" id="copy-status">
        {s.copyStatus}
      </StatusLine>
    </details>
  );
}

/** Switch account, and Log out naming the account. */
function Actions({ account }: Pick<AccountModels, 'account'>) {
  const { signOut } = accountCard(useViewModel(account));
  return (
    <div className="profile-actions">
      <Button variant="secondary" id="switch-account" onClick={() => account.switchAccount()}>
        Switch account
      </Button>
      <Button
        variant="secondary"
        className="sign-out"
        id="sign-out"
        title={signOut.title}
        onClick={() => account.signOut()}
      >
        {signOut.text}
      </Button>
    </div>
  );
}

/** What the account page is given. */
interface AccountPageProps extends AccountModels {
  /** The command palette is in view instead. */
  hidden: boolean;
}

/** The account page. */
export function AccountPage({ hidden, ...m }: AccountPageProps) {
  const { offer } = useViewModel(m.imports);
  if (offer)
    return (
      <section id="profile-section" hidden={hidden}>
        <ImportGroup vm={m.imports} />
      </section>
    );
  return (
    <section id="profile-section" hidden={hidden} aria-labelledby="account-name">
      <AccountCardView vm={m.account} />
      <SyncGroup vm={m.sync} />
      <ImportGroup vm={m.imports} />
      <PersonaGroup vm={m.profile} />
      <DeviceGroup vm={m.profile} />
      <Advanced dialog={m.dialog} reconnect={m.reconnect} />
      <Actions account={m.account} />
    </section>
  );
}
