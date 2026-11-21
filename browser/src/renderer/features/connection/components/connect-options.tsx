/**
 * The first-run screen: every way to connect on one page, laid out like a familiar sign-in page, so
 * people spot the account or app they already have without opening anything first.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Icon } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import { BrandMark, type BrandLogo } from './brand-marks.tsx';

/** The AI apps Oya can plug into, by the agent id main reports. */
const APPS: {
  /** The agent id main reports. */ id: string;
  /** The name people know it by. */ name: string;
  /** Its logo. */ logo?: BrandLogo;
}[] = [
  { id: 'claude-code', name: 'Claude Code', logo: 'claude' },
  { id: 'codex', name: 'Codex', logo: 'openai' },
  { id: 'cursor', name: 'Cursor', logo: 'cursor' },
  { id: 'vscode', name: 'VS Code', logo: 'vscode' },
];

/** A labelled divider between groups of options. */
function Divider({ label }: { /** The words in the middle of the line. */ label: string }) {
  return <p className="connect-divider">{label}</p>;
}

/** The two account sign-ins, as full-width "Continue with" buttons. */
function Accounts({ vm }: { /** Setup owner. */ vm: SetupViewModel }) {
  return (
    <div className="connect-accounts">
      <button type="button" className="connect-account" onClick={() => void vm.onboarding.connectChatgpt()}>
        <BrandMark logo="openai" />
        Continue with ChatGPT
        <small>Uses your ChatGPT plan</small>
      </button>
      <button type="button" className="connect-account" onClick={() => void vm.onboarding.technical.signIn()}>
        <BrandMark logo="oya" />
        Continue with Oya
        <small>Your Oya control panel, for developers</small>
      </button>
    </div>
  );
}

/** One button per AI app, each opening that app's connect steps. */
function Apps({ vm }: { /** Setup owner. */ vm: SetupViewModel }) {
  return (
    <div className="connect-apps">
      {APPS.map((app) => (
        <button
          type="button"
          className="connect-app"
          key={app.id}
          onClick={() => vm.onboarding.openDetail('agents', app.id)}
        >
          {app.logo && <BrandMark logo={app.logo} />}
          {app.name}
        </button>
      ))}
    </div>
  );
}

/** The less common routes, as plain rows. */
function MoreWays({ vm }: { /** Setup owner. */ vm: SetupViewModel }) {
  return (
    <div className="connect-more">
      <button type="button" className="connect-row" onClick={() => vm.onboarding.openDetail('model')}>
        <span>
          Use your own AI key <small>Gemini, Claude, OpenRouter</small>
        </span>
        <Icon name="chevron" />
      </button>
      <button type="button" className="connect-row" onClick={() => vm.show('manual')}>
        <span>Connect to my own server</span>
        <Icon name="chevron" />
      </button>
    </div>
  );
}

/** Oya sign-in progress and failure, shown under the buttons so every option stays in view. */
function SignInStatus({ vm }: { /** Setup owner. */ vm: SetupViewModel }) {
  const flow = vm.onboarding.technical;
  const s = useViewModel(flow);
  if (s.busy)
    return (
      <div role="status" className="connect-status">
        <p>{s.progress}</p>
        <Button onClick={() => flow.cancel()}>Cancel sign-in</Button>
      </div>
    );
  if (!s.error) return null;
  return (
    <div className="setup-error" role="alert">
      <p>{s.error}</p>
      <Button onClick={() => void flow.signIn()}>Try again</Button>
    </div>
  );
}

/** Every way to connect, at a glance. While any sign-in runs, every option is off, so none can start a second one. */
export function ConnectOptions({ vm }: { /** Setup owner. */ vm: SetupViewModel }) {
  const technical = useViewModel(vm.onboarding.technical);
  const flow = useViewModel(vm.onboarding);
  const busy = technical.busy || flow.busy;
  return (
    <>
      <h1 tabIndex={-1}>Connect the browser your way to enable AI mode</h1>
      <fieldset className="connect-options" disabled={busy}>
        <Accounts vm={vm} />
        <Divider label="or use your Claude plan or another AI app on this computer" />
        <Apps vm={vm} />
        <Divider label="or" />
        <MoreWays vm={vm} />
      </fieldset>
      <SignInStatus vm={vm} />
    </>
  );
}
