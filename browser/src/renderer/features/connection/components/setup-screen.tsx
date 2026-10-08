/**
 * The full-page welcome and sign-in screen (index.html's .setup-screen): what
 * the browser is for, and the panel that switches between start, waiting and
 * manual. The panel's views are in setup-views.tsx.
 */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Orb } from '../../../ui/index.ts';
import './setup-screen.css';
import type { SetupView, SetupViewModel } from '../view-models/setup-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import { ManualView, StartView, WaitingView } from './setup-views.tsx';

/** Focuses the first field (or else button) of the view shown, as the view changes. */
function useFocusView(view: SetupView) {
  const panel = useRef<HTMLElement>(null);
  useEffect(() => {
    const shown = panel.current?.querySelector(`.welcome-view[data-view="${view}"]`);
    (shown?.querySelector('input') ?? shown?.querySelector('button'))?.focus();
  }, [view]);
  return panel;
}

/** The left side: what Oya Browser is for, in three steps. */
function WelcomeStage() {
  return (
    <section className="welcome-stage" aria-labelledby="welcome-title">
      <Orb size="lg" className="brand-mark" />
      <div className="eyebrow">Oya Browser</div>
      <h1 id="welcome-title">
        Sign in once.
        <br />
        <em>Oya</em> takes it from&nbsp;here.
      </h1>
      <p className="welcome-lede">
        A real browser that is yours. Give Oya a task and it works with your logins. Take the wheel back any time.
      </p>
      <ol className="welcome-steps">
        <li>
          <span className="welcome-num">01</span>
          <strong>Sign in</strong>
          <span>Link this browser to your Oya workspace.</span>
        </li>
        <li>
          <span className="welcome-num">02</span>
          <strong>Log in to your sites</strong>
          <span>Here, as usual. Agents reuse those sessions.</span>
        </li>
        <li>
          <span className="welcome-num">03</span>
          <strong>Hand it to Oya</strong>
          <span>Type a task on the start page, or drive it from code, MCP and Playwright.</span>
        </li>
      </ol>
    </section>
  );
}

/** The welcome screen. */
export function SetupScreen({ vm }: ViewProps<SetupViewModel>) {
  const s = useViewModel(vm);
  const panel = useFocusView(s.view);
  return (
    <div className="setup-screen" data-view={s.view}>
      {/* The window moves by this strip (src/main/shell/window.ts hides the title bar). */}
      <div className="window-drag" aria-hidden="true"></div>
      <WelcomeStage />
      <section className="setup-card welcome-panel" aria-label="Connect this browser" ref={panel}>
        <StartView vm={vm} />
        <WaitingView vm={vm} />
        <ManualView vm={vm} />
        <div role="alert" className="setup-error" id="setup-error">
          {s.error}
        </div>
        <Button type="button" variant="secondary" onClick={() => void vm.makeDefaultBrowser()}>
          Make Oya Browser default…
        </Button>
        <Button type="button" className="welcome-skip" id="btn-skip" onClick={() => vm.skip()}>
          Just browse for now
        </Button>
      </section>
    </div>
  );
}
