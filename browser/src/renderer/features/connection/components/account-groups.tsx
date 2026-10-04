/**
 * The account page's groups: the account card, Sync, Imported logins,
 * Browsing as, and This device. Each reads its own ViewModel.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { accountCard, type AccountViewModel } from '../view-models/account-view-model.ts';
import { canSync, syncLine, type SyncViewModel } from '../view-models/sync-view-model.ts';
import { deviceText, personaText, type ProfileViewModel } from '../view-models/profile-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import { Button, StatusLine, TimeAgo } from '../../../ui/index.ts';
import { NAME_MAX_LENGTH } from '../model/constants.ts';

/** Avatar, name or email, project and plan, and the connection. */
export function AccountCardView({ vm }: ViewProps<AccountViewModel>) {
  const card = accountCard(useViewModel(vm));
  return (
    <div className="account-card">
      <div className="account-avatar" id="account-avatar" aria-hidden="true">
        {card.initial}
      </div>
      <div className="account-who">
        <h3 id="account-name">{card.title}</h3>
        <p id="account-email" hidden={!card.email}>
          {card.email}
        </p>
        <p className="account-meta" id="account-meta">
          {card.project}
          {card.plan ? <span className="plan-badge">{card.plan}</span> : null}
        </p>
        <StatusLine className="account-status" id="account-status" data-state={card.connection}>
          <span className="status-dot" aria-hidden="true"></span>
          <span id="account-status-text">{card.connectionText}</span>
        </StatusLine>
      </div>
    </div>
  );
}

/** When the logins last reached the server, and Sync now. */
export function SyncGroup({ vm }: ViewProps<SyncViewModel>) {
  const s = useViewModel(vm);
  const line = syncLine(s);
  return (
    <div className="profile-group" role="group" aria-labelledby="sync-label">
      <h4 className="profile-label" id="sync-label">
        Sync
      </h4>
      <div className="profile-row">
        <div className="row-text">
          <p className="row-title" id="sync-title">
            {line.lead}
            {line.at ? <TimeAgo at={line.at} /> : null}
            {line.count}
          </p>
          <p className="quiet" id="sync-detail">
            Kept for your agents and cloud browsers.
          </p>
        </div>
        <Button variant="secondary" id="sync-now" disabled={!canSync(s)} onClick={() => void vm.save()}>
          Sync now
        </Button>
      </div>
      <StatusLine className="quiet profile-note" id="profile-save-status">
        {s.note}
      </StatusLine>
    </div>
  );
}

/** The profile this browser browses as. */
export function PersonaGroup({ vm }: ViewProps<ProfileViewModel>) {
  const persona = personaText(useViewModel(vm).profileName);
  return (
    <div className="profile-group" role="group" aria-labelledby="persona-label">
      <h4 className="profile-label" id="persona-label">
        Browsing as
      </h4>
      <p className="row-title" id="persona-name">
        {persona.name}
      </p>
      <p className="quiet" id="fp-content">
        {persona.detail}
      </p>
    </div>
  );
}

/** This device: its name (saved as it is changed; Enter commits) and what sites see it as. */
export function DeviceGroup({ vm }: ViewProps<ProfileViewModel>) {
  const s = useViewModel(vm);
  return (
    <div className="profile-group" role="group" aria-labelledby="device-label">
      <h4 className="profile-label" id="device-label">
        This device
      </h4>
      <input
        id="profile-name"
        className="device-name"
        aria-label="Name of this browser"
        autoComplete="off"
        spellCheck={false}
        maxLength={NAME_MAX_LENGTH}
        value={s.name}
        onChange={(event) => vm.type(event.target.value)}
        onBlur={() => void vm.rename()}
        onKeyDown={(event) => event.key === 'Enter' && event.currentTarget.blur()}
      />
      <p className="quiet" id="profile-device" title={s.device ? 'What sites see this browser as' : undefined}>
        {deviceText(s.device)}
      </p>
      <StatusLine className="quiet profile-note" id="profile-name-status">
        {s.nameStatus}
      </StatusLine>
    </div>
  );
}
