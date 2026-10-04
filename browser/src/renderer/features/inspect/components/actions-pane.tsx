/**
 * The Actions pane (`#pane-actions`): page actions, the element row (number
 * and text, Click, Hover, Type, and the elements to pick from), More (Go,
 * Press, Wait, Click at), tab actions, and the result. The pane is marked busy
 * while an action runs.
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { ActionsViewModel } from '../view-models/actions-view-model.ts';
import { ActionButton as Btn, ActionElements, ActionInput as Field, ActionResultBox } from './action-controls.tsx';
import './inspect.css';

/** What the Actions pane is given. */
export interface ActionsPaneProps {
  /** The pane. */
  actions: ActionsViewModel;
  /** The workspace panel: the pane in view. */
  panel: RendererServices['panel'];
}

/** More: Go, Press, Wait and Click at, each with its fields. */
function MoreActions({ vm }: { /** The pane. */ vm: ActionsViewModel }) {
  return (
    <details className="studio-details action-more">
      <summary>More</summary>
      <div className="action-input-row">
        <Field vm={vm} id="action-url" label="Address" placeholder="Address" />
        <Btn vm={vm} action="navigate" small>
          Go
        </Btn>
      </div>
      <div className="action-input-row">
        <Field vm={vm} id="action-key" label="Key to press" placeholder="Key: Enter, Escape, Tab" />
        <Btn vm={vm} action="press-key" small>
          Press
        </Btn>
      </div>
      <div className="action-input-row">
        <Field vm={vm} id="action-wait-sel" label="CSS selector to wait for" placeholder="CSS selector to wait for" />
        <Btn vm={vm} action="wait" small>
          Wait
        </Btn>
      </div>
      <div className="action-input-row">
        <Field vm={vm} id="action-cx" label="X position" placeholder="X" number />
        <Field vm={vm} id="action-cy" label="Y position" placeholder="Y" number />
        <Btn vm={vm} action="click-coords" small>
          Click at
        </Btn>
      </div>
    </details>
  );
}

/** The element row: its number and text, then Click, Hover and Type. */
function ElementRow({ vm }: { /** The pane. */ vm: ActionsViewModel }) {
  return (
    <>
      <div className="action-label">Element</div>
      <div className="action-input-row">
        <Field vm={vm} id="action-click-id" label="Element number" placeholder="#" number />
        <Field vm={vm} id="action-type-text" label="Text to type" placeholder="Text to type" />
      </div>
      <div className="action-buttons">
        <Btn vm={vm} action="click" small>
          Click
        </Btn>
        <Btn vm={vm} action="hover" small>
          Hover
        </Btn>
        <Btn vm={vm} action="type" small>
          Type
        </Btn>
      </div>
      <ActionElements vm={vm} />
    </>
  );
}

/** The Actions pane, shown while the panel is on it. */
export function ActionsPane({ actions: vm, panel }: ActionsPaneProps) {
  const { pane } = useViewModel(panel);
  const { running } = useViewModel(vm);
  const cls = ['dev-pane', pane === 'actions' && 'active', running && 'busy'].filter(Boolean).join(' ');
  return (
    <div className={cls} id="pane-actions">
      <div className="actions-scroll">
        <div className="action-label">Page</div>
        <div className="actions-grid">
          <Btn vm={vm} action="analyze">
            Analyze
          </Btn>
          <Btn vm={vm} action="screenshot">
            Screenshot
          </Btn>
          <Btn vm={vm} action="reload">
            Reload
          </Btn>
          <Btn vm={vm} action="scroll-down">
            Scroll down
          </Btn>
          <Btn vm={vm} action="scroll-up">
            Scroll up
          </Btn>
        </div>
        <ElementRow vm={vm} />
        <MoreActions vm={vm} />
        <div className="action-label">Tabs</div>
        <div className="actions-grid">
          <Btn vm={vm} action="list-tabs">
            List tabs
          </Btn>
          <Btn vm={vm} action="new-tab">
            New tab
          </Btn>
        </div>
      </div>
      <ActionResultBox vm={vm} />
    </div>
  );
}
