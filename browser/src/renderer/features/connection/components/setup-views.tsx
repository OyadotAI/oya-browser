/**
 * The welcome panel's three views: start (Sign in with Oya), waiting (for the
 * sign-in to finish in the web browser) and manual (server, key and name).
 * The screen shows one at a time by its data-view (setup-screen.css).
 */
import type { KeyboardEvent, ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Orb, TextField } from '../../../ui/index.ts';
import type { SetupField, SetupViewModel } from '../view-models/setup-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import { TEXT } from '../model/constants.ts';

/** One labelled field of the manual form. */
interface FieldProps extends ViewProps<SetupViewModel> {
  /** The state field it edits. */
  field: SetupField;
  /** The input's id. */
  id: string;
  /** Its label. */
  label: string;
  /** Its input type. */
  type: 'text' | 'password';
  /** Its placeholder. */
  placeholder: string;
  /** The hint under it, if any. */
  hint?: ReactNode;
}

/** A labelled field; Enter connects. */
function Field({ vm, field, id, label, type, placeholder, hint }: FieldProps) {
  const value = useViewModel(vm)[field];
  const onKeyDown = (event: KeyboardEvent) => event.key === 'Enter' && void vm.connect();
  const spell = type === 'text' ? false : undefined;
  return (
    <TextField
      id={id}
      label={label}
      hint={hint}
      type={type}
      placeholder={placeholder}
      spellCheck={spell}
      value={value}
      onChange={(event) => vm.edit(field, event.target.value)}
      onKeyDown={onKeyDown}
    />
  );
}

/** Start: one-click sign-in, or the manual form. */
export function StartView({ vm }: ViewProps<SetupViewModel>) {
  return (
    <div className="welcome-view" data-view="start">
      <h2>Get started</h2>
      <p className="sub">Sign-in opens in your web browser. There is no key to copy.</p>
      <button className="btn-connect setup-primary" id="btn-signin" onClick={() => void vm.signIn()}>
        Sign in with Oya
        <span className="welcome-arrow" aria-hidden="true">
          →
        </span>
      </button>
      <div className="welcome-or">
        <span>or</span>
      </div>
      <button className="btn-skip setup-secondary" id="btn-manual" onClick={() => vm.show('manual')}>
        Connect with an API key
      </button>
      <p className="welcome-note">Self-hosting? Use your own server&apos;s address and key.</p>
    </div>
  );
}

/** Waiting: the sign-in finishes in the web browser, and this screen closes on its own. */
export function WaitingView({ vm }: ViewProps<SetupViewModel>) {
  return (
    <div className="welcome-view" data-view="waiting" id="setup-waiting" role="status">
      <Orb size="lg" state="thinking" className="welcome-orbit" />
      <h2>Finish in your web browser</h2>
      <p className="sub">
        Sign in there, then click <em>Connect</em> when this browser asks. This screen closes on its own.
      </p>
      <Button type="button" id="btn-signin-again" onClick={() => void vm.signIn()}>
        Open the sign-in page again
      </Button>
      <Button type="button" data-view-go="start" onClick={() => vm.show('start')}>
        Back
      </Button>
    </div>
  );
}

/** Manual: the server address, API key and browser name, for self-hosters. */
export function ManualView({ vm }: ViewProps<SetupViewModel>) {
  const busy = useViewModel(vm).busy;
  const keyHint = (
    <div className="hint">
      From your workspace&apos;s dashboard.{' '}
      <Button type="button" id="open-console" onClick={() => vm.openConsole()}>
        Open it
      </Button>
    </div>
  );
  return (
    <div className="welcome-view" data-view="manual">
      <Button type="button" className="welcome-back" data-view-go="start" onClick={() => vm.show('start')}>
        ← Back
      </Button>
      <h2>Connect with an API key</h2>
      <Field
        vm={vm}
        field="server"
        id="cfg-server"
        label="Server address"
        type="text"
        placeholder="wss://oyabrowser.com/ws"
      />
      <Field
        vm={vm}
        field="apiKey"
        id="cfg-key"
        label="API key"
        type="password"
        placeholder="Enter API key"
        hint={keyHint}
      />
      <Field
        vm={vm}
        field="name"
        id="cfg-name"
        label="Browser name"
        type="text"
        placeholder="Oya Browser"
        hint={<div className="hint">How this browser shows up in the dashboard</div>}
      />
      <button className="btn-connect setup-primary" id="btn-connect" disabled={busy} onClick={() => void vm.connect()}>
        {busy ? TEXT.connecting : TEXT.connect}
      </button>
    </div>
  );
}
