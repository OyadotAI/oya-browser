/** Instance-matched configuration lights and native launch actions make the local-agent route self-guided. */
import { useEffect } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { LocalAgent } from '../../../../shared/ipc.ts';
import type { AgentsViewModel } from '../view-models/agents-view-model.ts';
import { FIRST_TASK } from '../model/setup-constants.ts';
import { SetupIllustration } from './setup-illustration.tsx';

/** Configuration lights have explicit text; opening an agent never counts as a browser test. */
function AgentStatus({
  agent,
}: {
  /** Main returned no credentials, only current-instance matching. */ agent: LocalAgent;
}) {
  const text =
    agent.installed === false ? 'Not installed' : agent.connected ? 'This browser configured' : 'Connect this browser';
  return (
    <span className="technical-agent-status">
      <span className="technical-light" data-ready={agent.connected} aria-hidden="true" />
      <span>{text}</span>
    </span>
  );
}
/** Every installed agent offers a launch and practical test, regardless of the agent's subscription. */
function AgentRow({
  agent,
  vm,
}: {
  /** Allowlisted native agent. */ agent: LocalAgent;
  /** Configuration and launch state. */ vm: AgentsViewModel;
}) {
  const s = useViewModel(vm);
  return (
    <li>
      <div className="technical-agent-name">
        <strong>{agent.name}</strong>
        <AgentStatus agent={agent} />
        <span className="setup-detail">
          {agent.installed === false ? 'Install it, then check again.' : 'Test not run'}
        </span>
      </div>
      <div className="technical-agent-actions">
        {agent.installed !== false && !agent.connected && (
          <Button variant="secondary" disabled={!!s.busy} onClick={() => void vm.toggle(agent.id)}>
            {s.busy === agent.id ? 'Connecting…' : 'Connect'}
          </Button>
        )}
        {agent.installed !== false && (
          <Button
            variant="secondary"
            disabled={!!s.opening || agent.launchable === false}
            onClick={() => void vm.open(agent.id)}
          >
            {s.opening === agent.id ? 'Opening…' : `Open ${agent.name}`}
          </Button>
        )}
        {agent.installed !== false && <Button onClick={() => vm.select(agent.id)}>Test prompt</Button>}
      </div>
    </li>
  );
}
/** A prompt is copyable and visible even if the OS cannot launch the installation. */
function AgentTest({ vm }: { /** Selected agent and public prompt status. */ vm: AgentsViewModel }) {
  const s = useViewModel(vm),
    agent = s.agents.find((item) => item.id === s.selected);
  if (!agent) return null;
  return (
    <section className="setup-first-task" aria-label={`Test ${agent.name}`}>
      <strong>Try this in {agent.name}</strong>
      <p>{FIRST_TASK}</p>
      {!agent.connected && <p>Connect this browser first, then restart the agent.</p>}
      <div className="technical-agent-actions">
        <Button
          variant="secondary"
          disabled={!!s.opening || agent.launchable === false}
          onClick={() => void vm.open(agent.id)}
        >
          Open {agent.name}
        </Button>
        <Button onClick={() => void vm.copyTest()}>{s.copied ? 'Prompt copied' : 'Copy test prompt'}</Button>
      </div>
      {s.launchError && (
        <p className="setup-error" role="alert">
          {s.launchError}
        </p>
      )}
    </section>
  );
}
/** Agent rows appear before decorative material and remain usable at the minimum native size. */
export function TechnicalAgents({
  vm,
  finish,
}: {
  /** Technical-only discovery state. */ vm: AgentsViewModel;
  /** Explicit browsing escape. */ finish: () => void;
}) {
  const s = useViewModel(vm);
  useEffect(() => {
    void vm.refresh();
  }, [vm]);
  return (
    <>
      <div className="technical-heading">
        <SetupIllustration scene="tools" />
        <h2 tabIndex={-1}>Connect a local agent</h2>
      </div>
      {s.loading ? (
        <p role="status">Checking installed agents…</p>
      ) : s.error ? (
        <p role="alert">{s.error}</p>
      ) : !s.agents.length ? (
        <p>Local agent setup is unavailable. Restart Oya and check again.</p>
      ) : (
        <ul className="technical-agent-list">
          {s.agents.map((agent) => (
            <AgentRow key={agent.id} agent={agent} vm={vm} />
          ))}
        </ul>
      )}
      {s.note && <p role="status">{s.note}</p>}
      <AgentTest vm={vm} />
      <div className="technical-agent-actions">
        <Button disabled={s.loading || !!s.busy} onClick={() => void vm.refresh()}>
          Check again
        </Button>
        <Button variant="secondary" onClick={finish}>
          Open browser
        </Button>
      </div>
    </>
  );
}
