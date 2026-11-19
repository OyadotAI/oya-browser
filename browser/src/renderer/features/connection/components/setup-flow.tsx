/** Each starting point reuses supported services and ends with an explicit next action. */
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import type { SetupPath } from '../view-models/onboarding-view-model.ts';
import type { AgentsViewModel } from '../view-models/agents-view-model.ts';
import type { ImportViewModel } from '../view-models/import-view-model.ts';
import { SetupAgents } from './setup-agents.tsx';
import { GuideSteps } from './setup-guide.tsx';
import { LibraryImportPanel } from './library-import-panel.tsx';
import { SetupChoice } from './setup-choice.tsx';
import type { Brand } from './brand-marks.tsx';
import { TechnicalFlow } from './technical-flow.tsx';

/** The requested three paths, in their intended order. */
const PATHS: {
  /** Stable path. */ id: SetupPath;
  /** Heading. */ title: string;
  /** Explanation. */ detail: string;
  /** The tools this path works with. */ brands: Brand[];
}[] = [
  {
    id: 'technical',
    title: 'Use an AI app on this computer',
    detail: 'You need one of these installed, an AI key, or an Oya account',
    brands: [{ name: 'Claude Code', logo: 'claude' }, { name: 'Codex' }, { name: 'Cursor', logo: 'cursor' }],
  },
  {
    id: 'nontechnical',
    title: 'Use the AI plan you already pay for',
    detail: 'You need a ChatGPT plan, or a Claude plan with Claude Code',
    brands: [{ name: 'ChatGPT' }, { name: 'Claude', logo: 'claude' }],
  },
  {
    id: 'expert',
    title: 'Use your Oya workspace',
    detail: 'You need an Oya account',
    brands: [{ name: 'Oya', logo: 'oya' }],
  },
];
/** Shared models for each path. */
interface Props {
  /** Manual and first-run progress. */
  vm: SetupViewModel;
  /** Installed agents. */
  agents: AgentsViewModel;
  /** Optional imports. */
  imports: ImportViewModel;
}

/** Exactly three native buttons precede credentials. */
function Choices({ vm }: Pick<Props, 'vm'>) {
  return (
    <>
      <h1 tabIndex={-1}>Connect the browser your way to enable AI mode</h1>
      <p className="sub">Pick the way that fits you.</p>
      <div className="setup-choices">
        {PATHS.map((p) => (
          <SetupChoice
            key={p.id}
            path={p.id}
            title={p.title}
            caption={p.detail}
            brands={p.brands}
            onClick={() => vm.onboarding.choose(p.id)}
          />
        ))}
      </div>
    </>
  );
}

/** Distinguishes workspace keys from model credentials. */
function Technical({ vm }: Pick<Props, 'vm'>) {
  return <TechnicalFlow vm={vm} />;
}

/** Subscription choices explain where tasks will run. */
function Assistant({ vm }: Pick<Props, 'vm'>) {
  return (
    <>
      <h2 tabIndex={-1}>Connect your assistant</h2>
      <p className="sub">Use the subscription you already have.</p>
      <div className="setup-provider-choices">
        <SetupChoice
          title="Sign in with ChatGPT"
          caption="You need a ChatGPT plan"
          brands={[{ name: 'ChatGPT' }]}
          onClick={() => void vm.onboarding.connectChatgpt()}
        />
        <SetupChoice
          title="Use Claude Code"
          caption="You need a Claude plan and Claude Code installed"
          brands={[{ name: 'Claude Code', logo: 'claude' }]}
          onClick={() => vm.onboarding.show('claude')}
        />
      </div>
    </>
  );
}

/** Experts pair before exploring their actual dashboard. */
function Expert({ vm }: Pick<Props, 'vm'>) {
  return (
    <>
      <h2 tabIndex={-1}>Connect your workspace</h2>
      <p className="sub">Sign in, then approve this browser.</p>
      <Button variant="primary" onClick={() => void vm.signIn()}>
        Connect workspace
      </Button>
      <Button variant="secondary" onClick={() => vm.show('manual')}>
        Use a server and Oya key
      </Button>
    </>
  );
}

/** Opening a sign-in URL never counts as a completed connection. */
function ProviderWaiting({ vm }: Pick<Props, 'vm'>) {
  const s = useViewModel(vm.onboarding);
  return (
    <>
      <h2 tabIndex={-1}>Approve access in ChatGPT</h2>
      <p className="sub">Approve access, then return here.</p>
      <p role="status">
        {s.error
          ? 'Connection did not finish.'
          : s.busy
            ? 'Waiting for ChatGPT approval…'
            : 'Waiting for browser connection…'}
      </p>
      <Button variant="secondary" onClick={() => vm.onboarding.cancel()}>
        Cancel and go back
      </Button>
    </>
  );
}

/** A verified socket and completed provider setup lead to the relevant next action. */
function Connected({ vm }: Pick<Props, 'vm'>) {
  const s = useViewModel(vm.onboarding);
  const { busy } = useViewModel(vm);
  return (
    <>
      <h2 tabIndex={-1}>{s.connected ? 'You’re connected.' : 'Connection lost'}</h2>
      <p className="sub">
        {!s.connected
          ? 'Reconnect to finish setup, or browse for now.'
          : s.chatgptReady
            ? 'Type your first task in Oya’s Ask panel.'
            : 'Your workspace can now use this browser.'}
      </p>
      <Button
        variant="primary"
        disabled={!s.connected}
        onClick={() => vm.onboarding.show(s.path === 'expert' ? 'guide' : 'optional')}
      >
        {s.path === 'expert' ? 'Walk through the control plane' : 'Continue'}
      </Button>
      {!s.connected && (
        <Button variant="secondary" disabled={busy} onClick={() => void vm.reconnect()}>
          Reconnect
        </Button>
      )}
    </>
  );
}

/** Neither import nor the system default-browser prompt gates first use. */
function Optional({ vm, imports }: Pick<Props, 'vm' | 'imports'>) {
  return (
    <>
      <h2 tabIndex={-1}>Make yourself at home</h2>
      <p className="sub">Optional. You can do this later in Settings.</p>
      <LibraryImportPanel vm={imports.library} />
      <Button variant="secondary" onClick={() => void vm.makeDefaultBrowser()}>
        Make Oya the default browser
      </Button>
      <Button variant="primary" onClick={() => vm.onboarding.finish()}>
        Start using Oya
      </Button>
    </>
  );
}

/** The guarded map keeps view dispatch explicit. */
function Current(props: Props) {
  const { vm, agents } = props;
  const s = useViewModel(vm.onboarding);
  const views = {
    choices: () => <Choices vm={vm} />,
    technical: () => <Technical vm={vm} />,
    assistant: () => <Assistant vm={vm} />,
    expert: () => <Expert vm={vm} />,
    local: () => <SetupAgents vm={agents} setup={vm.onboarding} />,
    claude: () => <SetupAgents vm={agents} setup={vm.onboarding} claude />,
    waiting: () => <ProviderWaiting vm={vm} />,
    connected: () => <Connected vm={vm} />,
    optional: () => <Optional {...props} />,
    guide: () => (
      <GuideSteps
        step={s.step}
        move={(delta) => vm.onboarding.move(delta)}
        finish={() => vm.onboarding.show('optional')}
        openConsole={() => vm.onboarding.openConsole()}
      />
    ),
  };
  return Object.hasOwn(views, s.page) ? views[s.page]() : null;
}

/** Shared failure and Back controls keep every path recoverable. */
export function SetupFlow(props: Props) {
  const { vm } = props;
  const s = useViewModel(vm.onboarding);
  // One back button at a time: card 1's inner screens have their own, one step back.
  const technical = useViewModel(vm.onboarding.technical);
  const inner = s.page === 'technical' && technical.view !== 'choices';
  return (
    <div className="setup-flow">
      {s.page !== 'choices' && !inner && (
        <Button className="welcome-back" onClick={() => vm.onboarding.choose(null)}>
          ← Back to start
        </Button>
      )}
      <Current {...props} />
      {s.error && (
        <div role="alert" className="setup-error">
          <p>{s.error}</p>
          <Button onClick={() => vm.onboarding.cancel()}>Try again</Button>
        </div>
      )}
      {!s.connected && ['guide', 'optional'].includes(s.page) && <ConnectionLost vm={vm} />}
    </div>
  );
}

/** Retrying from the walkthrough preserves its current position. */
function ConnectionLost({ vm }: Pick<Props, 'vm'>) {
  const { busy } = useViewModel(vm);
  return (
    <div role="alert">
      <p>Connection lost. Reconnect to continue, or browse for now.</p>
      <Button disabled={busy} variant="secondary" onClick={() => void vm.reconnect()}>
        Reconnect
      </Button>
    </div>
  );
}
