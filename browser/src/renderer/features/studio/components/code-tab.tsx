/**
 * The Code tab: the generated Playwright module, numbered, with the selected
 * step's line highlighted; copy and save it; save or open the workflow as
 * JSON; and the diagnostics preview and export.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import { codeLines, isDisabled, type ControlId, type StudioState } from '../model/studio-model.ts';
import type { ReactNode } from 'react';
import { SlotStatus } from './fields.tsx';
import type { StudioPartProps } from './record-pane.tsx';
import { Button } from '../../../ui/index.ts';

/** The Code tab. */
export function CodeTab({ vm, state }: StudioPartProps) {
  const actions = vm.actions;
  return (
    <>
      <CodeView vm={vm} state={state} />
      <div className="export-actions">
        <ExportButton id="record-copy" state={state} onClick={() => actions.copy()}>
          Copy code
        </ExportButton>
        <ExportButton id="record-download" state={state} onClick={() => actions.download()}>
          Save module
        </ExportButton>
      </div>
      <div className="export-actions">
        <ExportButton id="record-json" state={state} onClick={() => actions.exportJson('oya')}>
          Save as JSON
        </ExportButton>
        <ExportButton id="record-chrome" state={state} onClick={() => actions.exportJson('chrome')}>
          Save for Chrome Recorder
        </ExportButton>
        <ExportButton id="record-import" state={state} onClick={() => actions.importJson()}>
          Open JSON
        </ExportButton>
      </div>
      <SlotStatus slot="code-result" message={state.messages['code-result']} />
      <Diagnostics vm={vm} state={state} />
    </>
  );
}

/** The generated code, numbered, with the selected step's line highlighted. */
export function CodeView({ state }: StudioPartProps) {
  const s = state.snapshot;
  const selectedLine = state.selected ? s?.mapping?.[state.selected] : undefined;
  return (
    <pre className="rec-code" id="record-code" tabIndex={0} aria-label="Generated Playwright code">
      {codeLines(s?.code ?? '').map((line, index) => (
        <span key={index} className={'code-line' + (selectedLine === index + 1 ? ' selected' : '')}>
          <span className="code-line-number" aria-hidden="true">
            {index + 1}
          </span>
          {line + '\n'}
        </span>
      ))}
    </pre>
  );
}

/** The diagnostics preview (the same redacted object the main process writes) and its export. */
export function Diagnostics({ vm, state }: StudioPartProps) {
  return (
    <details className="studio-details">
      <summary>Diagnostics</summary>
      <p className="studio-hint">
        Network metadata and console severity only. Request bodies, headers, cookies and page text are left out.
      </p>
      <pre id="support-preview" tabIndex={0}>
        {JSON.stringify(state.snapshot?.support ?? {}, null, C.JSON_INDENT)}
      </pre>
      <Button variant="secondary" id="support-export" onClick={() => void vm.actions.support()}>
        Export diagnostics
      </Button>
    </details>
  );
}

/** What an export button is. */
export interface ExportButtonProps {
  /** Its id, which names its disabled rule. */
  id: ControlId;
  /** The studio's state, for that rule. */
  state: StudioState;
  /** What a press does. */
  onClick: () => Promise<void>;
  /** Its words. */
  children: ReactNode;
}

/** A button under the code, off when its rule says so. */
export function ExportButton({ id, state, onClick, children }: ExportButtonProps) {
  return (
    <Button variant="secondary" id={id} disabled={isDisabled(state, id)} onClick={() => void onClick()}>
      {children}
    </Button>
  );
}
