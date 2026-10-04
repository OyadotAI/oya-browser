/**
 * The Record pane (#pane-record): the workflow header, the record and
 * test-run controls, what blocks a run, the save card, the studio tabs and
 * their panels. The stylesheet shows each part per stage from
 * #pane-record[data-stage].
 */
import { useRef, type KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Tab, TabList } from '../../../ui/index.ts';
import './studio.css';
import { STUDIO_TABS, TAB_LABELS } from '../model/constants.ts';
import { studioMode, type StudioState } from '../model/studio-model.ts';
import type { StudioViewModel } from '../view-models/studio-view-model.ts';
import { SlotStatus } from './fields.tsx';
import { StudioHeader, RecordControls, StudioNotes } from './studio-header.tsx';
import { FinishCard } from './finish-card.tsx';
import { StepsTab } from './steps-tab.tsx';
import { RunTab } from './run-tab.tsx';
import { CodeTab } from './code-tab.tsx';

/** What the Record pane is drawn from. */
export interface RecordPaneProps {
  /** The studio. */
  vm: StudioViewModel;
  /** The panel shows this pane. */
  active?: boolean;
  /** The page is watch-only and control cannot be taken, so the record button is refused (from the control feature). */
  controlBlocked?: boolean;
}

/** What every part of the pane is drawn from. */
export interface StudioPartProps {
  /** The studio. */
  vm: StudioViewModel;
  /** Its state now. */
  state: StudioState;
}

/** The Record pane. */
export function RecordPane({ vm, active = false, controlBlocked = false }: RecordPaneProps) {
  const state = useViewModel(vm);
  const hidden = (tab: string) => state.tab !== tab;
  return (
    <section
      className={active ? 'dev-pane active' : 'dev-pane'}
      id="pane-record"
      data-stage={studioMode(state).stage}
      aria-label="Workflow workspace"
    >
      <StudioHeader vm={vm} state={state} />
      <RecordControls vm={vm} state={state} controlBlocked={controlBlocked} />
      <StudioNotes vm={vm} state={state} />
      <SlotStatus slot="record-result" message={state.messages['record-result']} />
      <FinishCard vm={vm} state={state} />
      <StudioTabs vm={vm} state={state} />
      <div className="studio-scroll">
        <section id="studio-steps" role="tabpanel" hidden={hidden('steps')}>
          <StepsTab vm={vm} state={state} />
        </section>
        <section id="studio-run" role="tabpanel" hidden={hidden('run')}>
          <RunTab vm={vm} state={state} />
        </section>
        <section id="studio-code" role="tabpanel" hidden={hidden('code')}>
          <CodeTab vm={vm} state={state} />
        </section>
      </div>
    </section>
  );
}

/** Steps, Run and Code; the arrows move between them and focus follows. */
export function StudioTabs({ vm, state }: StudioPartProps) {
  const list = useRef<HTMLDivElement>(null);
  const onKeyDown = (event: KeyboardEvent) => {
    const tab = vm.tabKey(event.key);
    if (!tab) return;
    event.preventDefault();
    list.current?.querySelector<HTMLElement>(`[data-studio="${tab}"]`)?.focus();
  };
  return (
    <TabList className="studio-tabs" aria-label="Workflow view" ref={list}>
      {STUDIO_TABS.map((tab) => (
        <Tab
          key={tab}
          selected={state.tab === tab}
          data-studio={tab}
          onClick={() => vm.selectTab(tab)}
          onKeyDown={onKeyDown}
        >
          {TAB_LABELS[tab]}
        </Tab>
      ))}
    </TabList>
  );
}
