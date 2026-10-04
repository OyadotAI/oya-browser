/**
 * The workspace panel's frame around the panes: the resize handle, the
 * heading with Oya's mark (which shows the agent's state) and the close
 * button, the tool tabs (Ask, Record, Routines, Inspect) with arrow-key
 * movement, and the Inspect sub-tabs with Clear. The panes come in as children.
 */
import { useEffect, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { isRovingKey, useRovingFocus, useViewModel } from '../../hooks/index.ts';
import { IconButton, Tab, TabList, type OrbState } from '../../ui/index.ts';
import './panel.css';
import { RendererConstants as C } from '../../core/constants.ts';
import type { PanelViewModel } from './panel-view-model.ts';
import { INSPECT_PANES, type Pane } from './constants.ts';

/** A tab: the pane it shows and what it says. */
interface PaneTab {
  /** The pane. */
  pane: Pane;
  /** The tab's label. */
  label: string;
}

/** What a part of the frame is given. */
interface PartProps {
  /** The panel. */
  panel: PanelViewModel;
}

/** The tool tabs, in order: their pane and label. */
const TOOL_TABS: readonly PaneTab[] = [
  { pane: 'chat', label: 'Ask' },
  { pane: 'record', label: 'Record' },
  { pane: 'routines', label: 'Routines' },
  { pane: 'actions', label: 'Inspect' },
];

/** The Inspect sub-tabs: their pane and label. */
const INSPECT_TABS: readonly PaneTab[] = [
  { pane: 'actions', label: 'Actions' },
  { pane: 'network', label: 'Activity' },
  { pane: 'source', label: 'Source' },
];

/** The Inspect tab's id; the other tabs are `tool-tab-<pane>`. */
const INSPECT_TAB_ID = 'inspect-tab';

/** Whether `pane` is one of the Inspect tools. */
const isInspect = (pane: Pane): boolean => INSPECT_PANES.includes(pane);

/** The id of the tab that labels `pane`. */
export const tabIdFor = (pane: Pane): string => (isInspect(pane) ? INSPECT_TAB_ID : 'tool-tab-' + pane);

/** What the frame is given. */
export interface PanelFrameProps {
  /** The panel. */
  panel: PanelViewModel;
  /** What Oya's mark in the heading shows (the Ask run's orb state). */
  orb: OrbState;
  /** The panes. */
  children: ReactNode;
}

/** Arrow keys, Home and End move between the tool tabs; the last is Inspect. */
function useToolTabKeys(panel: PanelViewModel) {
  return useRovingFocus((index) =>
    index === TOOL_TABS.length - 1 ? panel.showInspect() : panel.show(TOOL_TABS[index].pane),
  );
}

/** One tool tab. */
function ToolTab({ panel, pane, label }: PartProps & PaneTab) {
  const { pane: shown } = useViewModel(panel);
  const inspect = pane === 'actions';
  const active = inspect ? isInspect(shown) : shown === pane;
  const controls = inspect && active ? shown : pane;
  return (
    <Tab
      className={active ? 'dev-tab active' : 'dev-tab'}
      data-pane={pane}
      id={inspect ? INSPECT_TAB_ID : 'tool-tab-' + pane}
      selected={active}
      aria-controls={'pane-' + controls}
      onClick={() => (inspect ? panel.showInspect() : panel.show(pane))}
    >
      {label}
    </Tab>
  );
}

/** The Inspect sub-tabs and Clear, shown while an Inspect tool is. */
function InspectNav({ panel }: PartProps) {
  const { pane } = useViewModel(panel);
  return (
    <div className="inspect-nav" id="inspect-nav" hidden={!isInspect(pane)}>
      {INSPECT_TABS.map(({ pane: tool, label }) => (
        <button
          key={tool}
          data-inspect={tool}
          className={tool === pane ? 'active' : undefined}
          aria-pressed={tool === pane}
          onClick={() => panel.show(tool)}
        >
          {label}
        </button>
      ))}
      <button className="dev-tab-btn" id="dev-clear" title="Clear this tool" onClick={() => panel.clear()}>
        Clear
      </button>
    </div>
  );
}

/** The panel's edge: drag or arrow keys to resize, a labelled separator. */
function ResizeHandle({ panel }: PartProps) {
  const { layout, dragging } = useViewModel(panel);
  const width = layout?.panelWidth ?? C.PANEL_MIN_WIDTH;
  const onDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (window.innerWidth < C.RESIZE_MIN_WINDOW || event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    panel.startDrag();
  };
  const end = () => panel.endDrag();
  useDragCursor(dragging, end);
  return (
    <div
      className={dragging ? 'dev-panel-resize dragging' : 'dev-panel-resize'}
      id="dev-panel-resize"
      tabIndex={0}
      role="separator"
      aria-label="Resize workspace tools"
      aria-orientation="vertical"
      aria-valuemin={C.PANEL_MIN_WIDTH}
      aria-valuemax={Math.min(C.PANEL_MAX_WIDTH, window.innerWidth - C.PAGE_MIN_WIDTH)}
      aria-valuenow={width}
      onPointerDown={onDown}
      onPointerMove={(event) => panel.dragTo(event.clientX, window.innerWidth)}
      onPointerUp={end}
      onPointerCancel={end}
      onLostPointerCapture={end}
      onKeyDown={(event) => resizeKey(event, panel)}
    />
  );
}

/** Resizes from the keyboard. */
function resizeKey(event: KeyboardEvent<HTMLDivElement>, panel: PanelViewModel): void {
  if (!isRovingKey(event.key)) return;
  event.preventDefault();
  const width = document.getElementById('dev-panel')?.getBoundingClientRect().width ?? C.PANEL_MIN_WIDTH;
  panel.resizeByKey(event.key, width);
}

/** While dragging, the whole page shows the resize cursor and selects nothing; a lost window ends the drag. */
function useDragCursor(dragging: boolean, end: () => void): void {
  useEffect(() => {
    document.body.style.cursor = dragging ? 'col-resize' : '';
    document.body.style.userSelect = dragging ? 'none' : '';
    if (!dragging) return;
    window.addEventListener('blur', end);
    return () => window.removeEventListener('blur', end);
  }, [dragging, end]);
}

/** Marks every pane a tab panel labelled by its tab, as the panes do not know their tabs. */
function usePaneLabels(root: React.RefObject<HTMLDivElement | null>): void {
  useEffect(() => {
    for (const pane of root.current?.querySelectorAll<HTMLElement>('.dev-pane') ?? []) {
      pane.setAttribute('role', 'tabpanel');
      pane.setAttribute('aria-labelledby', tabIdFor(pane.id.slice('pane-'.length) as Pane));
    }
  });
}

/** The heading: Oya's mark and the close button. */
function ToolHeading({ panel, orb }: PartProps & Pick<PanelFrameProps, 'orb'>) {
  const close = async () => {
    await panel.toggle();
    document.getElementById('btn-dev')?.focus();
  };
  return (
    <header className="tool-heading">
      <span className="tool-title">
        <svg className="oya-mark panel-mark" id="panel-orb" viewBox="0 0 512 512" aria-hidden="true" data-state={orb}>
          <circle className="mark-back" cx="276" cy="276" r="160" />
          <circle className="mark-front" cx="236" cy="236" r="160" />
        </svg>
        Oya
      </span>
      <IconButton
        className="nav-btn"
        id="tools-close"
        icon="close"
        aria-label="Close agent panel"
        data-icon="close"
        onClick={close}
      />
    </header>
  );
}

/** The workspace panel. */
export function PanelFrame({ panel, orb, children }: PanelFrameProps) {
  const { open, layout } = useViewModel(panel);
  const root = useRef<HTMLDivElement>(null);
  usePaneLabels(root);
  const onTabKey = useToolTabKeys(panel);
  const shown = (layout?.progress ?? 0) > 0;
  return (
    <div className={shown ? 'dev-panel open' : 'dev-panel'} id="dev-panel" ref={root} inert={!open}>
      <ResizeHandle panel={panel} />
      <ToolHeading panel={panel} orb={orb} />
      <TabList className="dev-panel-header" aria-label="Agent tools" onKeyDown={onTabKey}>
        {TOOL_TABS.map(({ pane, label }) => (
          <ToolTab key={pane} panel={panel} pane={pane} label={label} />
        ))}
      </TabList>
      <InspectNav panel={panel} />
      {children}
    </div>
  );
}
