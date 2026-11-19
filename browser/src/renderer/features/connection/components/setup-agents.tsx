/** Local-agent guidance never mistakes installed configuration for live subscription authentication. */
import { useEffect } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { AgentsViewModel } from '../view-models/agents-view-model.ts';
import type { OnboardingViewModel } from '../view-models/onboarding-view-model.ts';
import { FIRST_TASK } from '../model/setup-constants.ts';

/** The same list renderer is shared by technical and subscription setup. */
interface AgentListProps {
  /** Shared discovery and mutation state. */
  vm: AgentsViewModel;
  /** Restricts the subscription path to its unmodified runtime. */
  claude: boolean;
}

/** Discovery, empty and configured states never imply that authentication was verified. */
function AgentList({ vm, claude }: AgentListProps) {
  const s = useViewModel(vm);
  const agents = s.agents.filter((agent) => !claude || agent.id === 'claude-code');
  if (s.loading) return <p role="status">Looking for agents on this computer…</p>;
  if (s.error) return <p role="alert">Could not check installed agents.</p>;
  if (!agents.length)
    return <p>{claude ? 'Install Claude Code first, then return here.' : 'No supported agents found.'}</p>;
  return (
    <ul className="setup-agent-list">
      {agents.map((agent) => (
        <li key={agent.id}>
          <strong>{agent.name}</strong>
          {agent.connected ? (
            <span>Oya added. Restart {agent.name}.</span>
          ) : (
            <Button disabled={!!s.busy} variant="secondary" onClick={() => void vm.toggle(agent.id)}>
              {s.busy === agent.id ? 'Adding Oya…' : `Connect ${agent.name}`}
            </Button>
          )}
        </li>
      ))}
    </ul>
  );
}

/** Claude Code owns its own subscription credentials and setup-token environment. */
function ClaudeInstructions({ setup }: { /** Fixed documentation actions. */ setup: OnboardingViewModel }) {
  return (
    <>
      <p className="sub">Then give Claude Code tasks as usual. It uses this browser.</p>
      <ol className="setup-mini-steps">
        <li>
          <strong>Sign in</strong>
          <span>Open Claude Code and sign in with your Claude account</span>
        </li>
        <li>
          <strong>Connect Oya</strong>
          <span>Press Connect below</span>
        </li>
        <li>
          <strong>Restart</strong>
          <span>Close Claude Code and open it again</span>
        </li>
      </ol>
      <details className="setup-help">
        <summary>Can’t sign in from a browser?</summary>
        <p>If Claude Code runs on a server or without a screen, follow the guide to sign it in another way.</p>
        <Button onClick={() => setup.help('claude-auth')}>Open the guide</Button>
      </details>
    </>
  );
}

/** Rechecks installation on entry and provides explicit error and retry states. */
export function SetupAgents({
  vm,
  setup,
  claude = false,
}: {
  /** Shared agent state. */ vm: AgentsViewModel;
  /** First-run navigation. */ setup: OnboardingViewModel;
  /** Show only the subscription runtime. */ claude?: boolean;
}) {
  const s = useViewModel(vm);
  useEffect(() => {
    void vm.refresh();
  }, [vm]);
  const agents = s.agents.filter((agent) => !claude || agent.id === 'claude-code');
  return (
    <>
      <h2 tabIndex={-1}>{claude ? 'Connect Claude Code' : 'Connect an AI app on this computer'}</h2>
      {claude ? <ClaudeInstructions setup={setup} /> : <p className="sub">Add Oya, then restart your agent.</p>}
      <AgentList vm={vm} claude={claude} />
      {s.note && <p role="status">{s.note}</p>}
      <Button disabled={s.loading || !!s.busy} onClick={() => void vm.refresh()}>
        Check again
      </Button>
      {claude && <Button onClick={() => setup.help('claude-install')}>Installation instructions</Button>}
      {agents.some((a) => a.connected) && (
        <div className="setup-first-task">
          <strong>Try this in {claude ? 'Claude Code' : 'your agent'}</strong>
          <p>{FIRST_TASK}</p>
          <Button variant="primary" onClick={() => setup.finish()}>
            Open browser
          </Button>
        </div>
      )}
    </>
  );
}
