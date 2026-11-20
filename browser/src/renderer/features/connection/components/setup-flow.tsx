/** Each starting point reuses supported services and ends with an explicit next action. */
import type { ReactElement } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import type { AgentsViewModel } from '../view-models/agents-view-model.ts';
import type { ImportViewModel } from '../view-models/import-view-model.ts';
import { GuideSteps } from './setup-guide.tsx';
import { LibraryImportPanel } from './library-import-panel.tsx';
import { ConnectOptions } from './connect-options.tsx';
import { TechnicalFlow } from './technical-flow.tsx';

/** Shared models for each path. */
interface Props {
  /** Manual and first-run progress. */
  vm: SetupViewModel;
  /** Installed agents. */
  agents: AgentsViewModel;
  /** Optional imports. */
  imports: ImportViewModel;
}

/** Distinguishes workspace keys from model credentials. */
function Technical({ vm }: Pick<Props, 'vm'>) {
  return <TechnicalFlow vm={vm} />;
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
  const { vm } = props;
  const s = useViewModel(vm.onboarding);
  // Pages the old cards opened (assistant, expert, claude, local) have no screen any more.
  const views: Partial<Record<typeof s.page, () => ReactElement>> = {
    choices: () => <ConnectOptions vm={vm} />,
    technical: () => <Technical vm={vm} />,
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
  return views[s.page]?.() ?? null;
}

/** Shared failure and Back controls keep every path recoverable. */
export function SetupFlow(props: Props) {
  const { vm } = props;
  const s = useViewModel(vm.onboarding);
  // One back button at a time: the app and key screens have their own; once signed in, there is no going back to sign-in.
  const technical = useViewModel(vm.onboarding.technical);
  const inner = s.page === 'technical' && technical.view !== 'choices';
  return (
    <div className="setup-flow">
      {!['choices', 'guide', 'optional'].includes(s.page) && !inner && (
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
