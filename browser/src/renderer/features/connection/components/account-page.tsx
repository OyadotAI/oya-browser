/**
 * The account page of the shell dialog (index.html's #profile-section), in the
 * order a person looks for things: who they are (the account card, with Switch
 * account and Log out on it); their logins (import from a browser, saved
 * passwords, sync); this browser (make it the default, the profile it browses
 * as, its name); and the server and browser id folded under Advanced.
 */
import type { ReactNode } from 'react';
import { SettingsNavigation, SettingsHeading } from './settings-navigation.tsx';
import type { SettingsPage } from '../view-models/shell-dialog-view-model.ts';
import { useViewModel } from '../../../hooks/index.ts';
import { TEXT } from '../model/constants.ts';
import type { AccountModels } from './shell-dialog.tsx';
import { AccountCardView, DeviceRow, PersonaRow, SyncRow } from './account-groups.tsx';
import { DefaultBrowserRow, ImportGroup, ImportRows } from './import-group.tsx';
import { SettingsSection } from './settings-list.tsx';
import { accountCard } from '../view-models/account-view-model.ts';
import { Button, StatusLine } from '../../../ui/index.ts';
import './account-page.css';
import './settings-layout.css';
import { SettingsSetupGuide } from './setup-guide.tsx';

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
      <Button variant="text" id="switch-account" onClick={() => account.switchAccount()}>
        Switch account
      </Button>
      <Button variant="text" className="sign-out" id="sign-out" title={signOut.title} onClick={() => account.signOut()}>
        {signOut.text}
      </Button>
    </div>
  );
}

/** What the account page is given. */
interface AccountPageProps extends AccountModels {
  /** The command palette is in view instead. */
  hidden: boolean;
  /** Appearance and page format keep their existing ViewModels. */
  preferences?: ReactNode;
}

/** General settings keep appearance, default-browser and device controls together. */
function General({
  preferences,
  ...m
}: AccountModels & {
  /** Existing live theme and page-format controls. */
  preferences?: ReactNode;
}) {
  return (
    <>
      <div className="settings-preferences">{preferences}</div>
      {m.setup && <SettingsSetupGuide openConsole={() => m.setup?.onboarding.openConsole()} />}
      <SettingsSection id="browser-label" label="Browser">
        <DefaultBrowserRow vm={m.imports} />
        <PersonaRow vm={m.profile} />
        <DeviceRow vm={m.profile} />
      </SettingsSection>
    </>
  );
}
/** Each section keeps its controls mounted while only the chosen section is visible. */
function SettingsContents({
  page,
  preferences,
  ...m
}: AccountModels & {
  /** Selected section. */
  page: SettingsPage;
  /** Existing appearance controls. */
  preferences?: ReactNode;
}) {
  return (
    <div className="settings-content">
      <SettingsHeading page={page} />
      <div id="settings-general" hidden={page !== 'general'}>
        <General {...m} preferences={preferences} />
      </div>
      <div id="settings-logins" hidden={page !== 'logins'}>
        <SettingsSection id="logins-label" label="Import & sync">
          <ImportRows vm={m.imports} />
          <SyncRow vm={m.sync} />
        </SettingsSection>
      </div>
      <div id="settings-account" hidden={page !== 'account'}>
        <AccountCardView vm={m.account} actions={<Actions account={m.account} />} />
        <Advanced dialog={m.dialog} reconnect={m.reconnect} />
      </div>
    </div>
  );
}
/** Settings navigation separates everyday preferences from logins and account maintenance. */
export function AccountPage({ hidden, preferences, ...m }: AccountPageProps) {
  const { offer } = useViewModel(m.imports);
  const { settingsPage: page } = useViewModel(m.dialog);
  if (offer)
    return (
      <section id="profile-section" hidden={hidden}>
        <ImportGroup vm={m.imports} />
      </section>
    );
  return (
    <section id="profile-section" hidden={hidden} aria-label="Browser settings">
      <div className="settings-layout">
        <SettingsNavigation page={page} choose={(page) => m.dialog.chooseSettings(page)} account={m.account} />
        <SettingsContents {...m} page={page} preferences={preferences} />
      </div>
    </section>
  );
}
