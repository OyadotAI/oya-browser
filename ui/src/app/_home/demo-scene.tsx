/** One synthetic task travels from a portal to a reusable playbook and a visible result. */
'use client';
import { Check, FileCode2, MessageSquareText, MousePointer2, Pause, Play, Repeat2 } from 'lucide-react';
import type { ReplayTask } from './content';
import type { DemoExample } from './demo-content';
import { PortalPane, useReplay } from './replay-showcase';
import { DemoDetails } from './demo-details';
import { HandoffControls } from './handoff-controls';
import { useHandoff } from './use-handoff';
import styles from './demo.module.css';

/** Four visible steps explain how a prompt becomes a repeatable artifact. */
const STEPS = [
  {
    title: 'Describe the task',
    body: 'Send work through MCP or your application.',
    icon: MessageSquareText,
    artifact: false,
  },
  { title: 'Record once', body: 'Oya records the agent’s portal actions.', icon: MousePointer2, artifact: false },
  {
    title: 'Save the playbook',
    body: 'Keep readable Playwright code with variable inputs.',
    icon: FileCode2,
    artifact: true,
  },
  {
    title: 'Replay new inputs',
    body: 'Call the saved workflow through the SDK, without a model.',
    icon: Repeat2,
    artifact: false,
  },
];

/** The reusable artifact names the selected task's file and inputs without requiring a drawer. */
function Artifact({ task }: { /** Existing selected task data. */ task: ReplayTask }) {
  return (
    <div className={styles.artifact} role="group" aria-label="Saved playbook">
      <p>
        Saved playbook · <code>{task.file}</code>
      </p>
      <span>Inputs: {task.fields.map(({ label }) => label).join(' · ')}</span>
    </div>
  );
}

/** A concrete synthetic request gives the workflow an understandable business entry point. */
function ExamplePrompt({ prompt }: { /** The request paired with the selected task. */ prompt: string }) {
  return (
    <div className={styles.prompt}>
      <span>Example prompt</span>
      <p>{prompt}</p>
    </div>
  );
}

/** The reusable playbook is a connected process rather than another wall of source code. */
function Process({
  task,
  done,
  paused,
}: {
  /** The chosen example. */ task: ReplayTask;
  /** Whether its result is ready. */ done: boolean;
  /** Execution is paused manually or by human ownership. */ paused: boolean;
}) {
  return (
    <div className={styles.process} id="steps">
      {STEPS.map(({ title, body, icon: Icon, artifact }) => (
        <div className={styles.processStep} key={title}>
          <span className={styles.processIcon}>
            <Icon size={19} aria-hidden="true" />
          </span>
          <div>
            <h3>{title}</h3>
            <p>{body}</p>
            {artifact && <Artifact task={task} />}
          </div>
        </div>
      ))}
      <div className={styles.result} role="status">
        <Check size={19} aria-hidden="true" />
        <span>{done ? task.result : paused ? 'Example workflow paused.' : 'Example workflow replaying…'}</span>
      </div>
    </div>
  );
}

/** Current execution state and the pause action available while the agent owns control. */
interface BarProps {
  /** Consistent visible execution state. */ status: string;
  /** Whether manual execution is paused. */ paused: boolean;
  /** Whether motion and ownership permit the pause/play control. */ controlsVisible: boolean;
  /** Toggles manual execution. */ toggle: () => void;
}

/** Keeps status text consistent with manual pause and human ownership. */
function SceneBar({ status, paused, controlsVisible, toggle }: BarProps) {
  return (
    <div className={styles.sceneBar}>
      <span>
        <i aria-hidden="true" />
        {status}, no model
      </span>
      {controlsVisible && (
        <button type="button" onClick={toggle}>
          {paused ? <Play size={14} aria-hidden="true" /> : <Pause size={14} aria-hidden="true" />}
          {paused ? 'Play replay' : 'Pause replay'}
        </button>
      )}
    </div>
  );
}

/** Remounting by task ID resets the replay and releases the previous task's interval. */
export function DemoScene({ example }: { /** The descriptor chosen in the scene's tab rail. */ example: DemoExample }) {
  const { task, prompt } = example;
  const handoff = useHandoff();
  const { ref, step, paused, still, toggle, resume, restart } = useReplay(task, handoff.owner !== 'running');
  const executionPaused = paused || handoff.owner !== 'running';
  const status = executionPaused ? 'Paused' : step === task.code.length ? 'Replayed' : 'Replaying';
  const advance = () => {
    if (handoff.owner === 'paused') resume();
    handoff.advance();
  };
  const reset = () => {
    handoff.reset();
    restart();
  };
  return (
    <figure ref={ref} className={styles.scene} aria-label={`Example replay: ${task.title}`}>
      <SceneBar
        status={status}
        paused={paused}
        controlsVisible={!still && handoff.owner === 'running'}
        toggle={toggle}
      />
      <ExamplePrompt prompt={prompt} />
      <div className={styles.stage}>
        <div className={styles.portalShell}>
          <div className={styles.portalBar}>
            <span className={styles.windowDots} aria-hidden="true">
              •••
            </span>
            <span>Example payer portal</span>
          </div>
          <PortalPane task={task} step={step} showResult={false} />
        </div>
        <Process task={task} done={step === task.code.length} paused={executionPaused} />
      </div>
      <HandoffControls
        owner={handoff.owner}
        executionPaused={executionPaused}
        events={handoff.events}
        advance={advance}
        restart={reset}
      />
      <DemoDetails task={task} step={step} />
    </figure>
  );
}
