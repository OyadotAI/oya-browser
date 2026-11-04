/** Three equally weighted technical routes replace the workspace-key-first setup. */
import { useViewModel } from '../../../hooks/index.ts';
import { useEffect } from 'react';
import { Button } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import { SetupChoice } from './setup-choice.tsx';
import { TechnicalAgents } from './technical-agents.tsx';
import { TechnicalModel } from './technical-model.tsx';
import './technical-setup.css';

/** Technical-only detail navigation preserves the other onboarding paths. */
export function TechnicalFlow({ vm }: { /** Existing manual connection and technical state. */ vm: SetupViewModel }) {
  const flow = vm.onboarding.technical;
  useEffect(() => () => flow.cancel(), [flow]);
  const s = useViewModel(flow);
  if (s.view === 'agents')
    return (
      <>
        <TechnicalBack vm={vm} />
        <TechnicalAgents vm={flow.agents} finish={() => vm.onboarding.finish()} />
      </>
    );
  if (s.view === 'model')
    return (
      <>
        <TechnicalBack vm={vm} />
        <TechnicalModel vm={flow.model} />
      </>
    );
  return <TechnicalChoices vm={vm} />;
}
/** Back cancels pending credentials instead of leaving hidden work behind the next route. */
function TechnicalBack({ vm }: { /** The technical view owns its active attempt. */ vm: SetupViewModel }) {
  return (
    <Button className="welcome-back" onClick={() => vm.onboarding.technical.show('choices')}>
      ← Technical options
    </Button>
  );
}
/** Each full row is the same native control with a compact explanatory picture. */
function TechnicalChoices({ vm }: { /** Shared setup owner. */ vm: SetupViewModel }) {
  const flow = vm.onboarding.technical;
  const s = useViewModel(flow);
  return (
    <>
      <h2 tabIndex={-1}>Connect Oya your way.</h2>
      <div className="technical-choices">
        <SetupChoice
          compact
          title="Sign in with Oya"
          caption="Connect your workspace automatically"
          scene="oya"
          disabled={s.busy}
          onClick={() => void flow.signIn()}
        />
        <SetupChoice
          compact
          title="Connect a local agent"
          caption="Claude Code, Codex, Cursor or VS Code"
          scene="tools"
          disabled={s.busy}
          onClick={() => flow.show('agents')}
        />
        <SetupChoice
          compact
          title="Use a model API key"
          caption="Gemini, Claude API or OpenRouter"
          scene="models"
          disabled={s.busy}
          onClick={() => flow.show('model')}
        />
      </div>
      {s.busy && (
        <>
          <p role="status">{s.progress}</p>
          <Button onClick={() => flow.cancel()}>Cancel sign-in</Button>
        </>
      )}
      {s.error && (
        <div className="setup-error" role="alert">
          <p>{s.error}</p>
          <Button onClick={() => void flow.signIn()}>Try again</Button>
        </div>
      )}
      <Button className="technical-advanced" disabled={s.busy} onClick={() => vm.show('manual')}>
        Advanced connection
      </Button>
    </>
  );
}
