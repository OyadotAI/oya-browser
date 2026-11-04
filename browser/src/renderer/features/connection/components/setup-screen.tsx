/** First run offers three starting points before exposing connection details. */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Orb } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import type { AgentsViewModel } from '../view-models/agents-view-model.ts';
import type { ImportViewModel } from '../view-models/import-view-model.ts';
import { ManualView, WaitingView } from './setup-views.tsx';
import { AccountChooser } from './account-chooser.tsx';
import { SetupFlow } from './setup-flow.tsx';
import './setup-screen.css';

/** Existing models share account recovery and explicit setup actions with Settings. */
interface Props {
  /** Manual form and account recovery. */
  vm: SetupViewModel;
  /** Local agent configuration. */
  agents: AgentsViewModel;
  /** Optional local library import. */
  imports: ImportViewModel;
}

/** Focus follows intentional navigation instead of jumping into a secret field. */
function useSetupFocus(key: string, path: string | null) {
  const panel = useRef<HTMLElement>(null);
  const previousPath = useRef<string | null>(null);
  useEffect(() => {
    if (path) previousPath.current = path;
    const choice =
      key.endsWith(':choices') && previousPath.current
        ? panel.current?.querySelector<HTMLElement>(`[data-path="${previousPath.current}"]`)
        : null;
    (choice ?? panel.current?.querySelector<HTMLElement>('h1, h2'))?.focus();
  }, [key, path]);
  return panel;
}

/** The panel remains scrollable at the native minimum window size. */
export function SetupScreen({ vm, agents, imports }: Props) {
  const s = useViewModel(vm);
  const flow = useViewModel(vm.onboarding);
  const technical = useViewModel(vm.onboarding.technical);
  const panel = useSetupFocus(`${s.view}:${flow.page}:${technical.view}`, flow.path);
  return (
    <main className="setup-screen" data-view={s.view} data-page={flow.page}>
      <div className="window-drag" aria-hidden="true" />
      <section className="setup-card welcome-panel" aria-label="Set up Oya Browser" ref={panel}>
        <div className="setup-brand">
          <Orb size="lg" className="brand-mark" />
          <p className="eyebrow">Oya Browser</p>
        </div>
        {s.view === 'accounts' ? (
          <AccountChooser vm={vm} />
        ) : s.view === 'manual' ? (
          <ManualView vm={vm} />
        ) : s.view === 'waiting' ? (
          <WaitingView vm={vm} />
        ) : (
          <SetupFlow vm={vm} agents={agents} imports={imports} />
        )}
        {s.error && (
          <p role="alert" className="setup-error">
            {s.error}
          </p>
        )}
        <Button type="button" className="welcome-skip" id="btn-skip" onClick={() => vm.skip()}>
          Just browse for now
        </Button>
      </section>
    </main>
  );
}
