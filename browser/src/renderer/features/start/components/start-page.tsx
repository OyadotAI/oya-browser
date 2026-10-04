/**
 * The Oya start page (index.html's #start-page): drawn by the shell where a
 * new tab's page would be. The task box's text is the view's own; Enter or
 * the send button hands it to the agent, Shift+Enter is a new line, and an
 * example runs as if typed. Each arrival replays the page's entrance and puts
 * the cursor in the task box.
 */
import { useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { IconButton, Orb } from '../../../ui/index.ts';
import './start-page.css';
import type { StartViewModel } from '../view-models/start-view-model.ts';
import { ARRIVING, EXAMPLES, SUBMIT_KEY, TEXT } from '../model/constants.ts';

/** What the start page is given. */
export interface StartPageProps {
  /** The start page's ViewModel. */
  vm: StartViewModel;
}

/** Replays the arrival and focuses the task box on each arrival. */
function useArrival(arrivals: number) {
  const page = useRef<HTMLElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    if (!arrivals || !page.current) return;
    page.current.classList.remove(ARRIVING);
    void page.current.offsetWidth; // a reflow forgets the finished animation
    page.current.classList.add(ARRIVING);
    input.current?.focus();
  }, [arrivals]);
  return { page, input };
}

/** Enter starts the task; Shift+Enter is a new line, and so is Enter mid-composition. */
const submits = (event: KeyboardEvent): boolean =>
  event.key === SUBMIT_KEY && !event.shiftKey && !event.nativeEvent.isComposing;

/** The example tasks under the box; one asks at once. */
function StartExamples({ vm }: StartPageProps) {
  return (
    <div className="start-examples" id="start-examples" aria-label={TEXT.examples}>
      {EXAMPLES.map(({ task: example, label }) => (
        <button key={label} type="button" data-task={example} onClick={() => vm.ask(example)}>
          {label}
        </button>
      ))}
    </div>
  );
}

/** The start page. */
export function StartPage({ vm }: StartPageProps) {
  const { home, arrivals } = useViewModel(vm);
  const { page, input } = useArrival(arrivals);
  const [task, setTask] = useState('');
  const send = () => vm.ask(task) && setTask('');
  return (
    <main className="start-page" id="start-page" aria-labelledby="start-title" hidden={!home} ref={page}>
      <div className="start-aura" aria-hidden="true">
        <i></i>
      </div>
      <div className="start-lattice" aria-hidden="true"></div>
      <div className="start-center">
        <Orb size="xl" id="start-orb" />
        <h1 className="start-title" id="start-title">
          What should I <em>do?</em>
        </h1>
        <p className="start-sub">{TEXT.sub}</p>
        <form className="start-ask" id="start-ask" onSubmit={(event) => (event.preventDefault(), send())}>
          <textarea
            id="start-input"
            rows={1}
            aria-label={TEXT.input}
            placeholder={TEXT.placeholder}
            spellCheck={false}
            ref={input}
            value={task}
            onChange={(event) => setTask(event.target.value)}
            onKeyDown={(event) => submits(event) && (event.preventDefault(), send())}
          />
          <IconButton
            className="start-go"
            id="start-go"
            type="submit"
            aria-label={TEXT.go}
            data-icon="send"
            icon="send"
          />
        </form>
        <StartExamples vm={vm} />
      </div>
      <footer className="start-foot">
        <kbd>⌘K</kbd> Commands <span aria-hidden="true">·</span> <kbd>⌘L</kbd> Address
      </footer>
    </main>
  );
}
