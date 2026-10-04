/**
 * The Routines pane (`#pane-routines`): the heading with New, the banner
 * (offline, or why the list could not be read), the editor card, and a card
 * per routine, or the invitation to make a first one.
 */
import { useCallback } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { RendererServices } from '../../../app/services.ts';
import { ROUTINES_TEXT } from '../model/constants.ts';
import { RoutineCard } from './routine-card.tsx';
import { RoutineEditor } from './routine-editor.tsx';
import type { RoutinesSnapshot } from '../model/types.ts';
import type { RoutinesViewModel } from '../view-models/routines-view-model.ts';
import { useMenuDismiss } from '../hooks/use-menu-dismiss.ts';
import { Button, StatusLine } from '../../../ui/index.ts';
import './routines.css';

/** What the Routines pane is given. */
export interface RoutinesPaneProps {
  /** The pane. */
  routines: RoutinesViewModel;
  /** The workspace panel: the pane in view. */
  panel: RendererServices['panel'];
}

/** What the banner says: offline, why the list could not be read, or nothing. */
const bannerText = ({ online, error }: RoutinesSnapshot): string =>
  !online ? ROUTINES_TEXT.offline : error ? ROUTINES_TEXT.loadError(error) : '';

/** The Routines pane, shown while the panel is on Routines. */
export function RoutinesPane({ routines: vm, panel }: RoutinesPaneProps) {
  const { pane } = useViewModel(panel);
  const state = useViewModel(vm);
  useMenuDismiss(
    state.menu !== null,
    useCallback(() => vm.closeMenu(), [vm]),
  );
  const banner = bannerText(state.snapshot);
  const list = state.snapshot.routines;
  return (
    <section className={pane === 'routines' ? 'dev-pane active' : 'dev-pane'} id="pane-routines" aria-label="Routines">
      <div className="routines-scroll">
        <div className="routines-head">
          <div>
            <h2 className="routines-title">Routines</h2>
            <p className="routines-sub">
              Prompts the agent runs on a schedule. They belong to this project and run while an Oya app on it is open.
            </p>
          </div>
          <Button
            type="button"
            variant="primary"
            className="routines-new"
            id="routine-new"
            onClick={() => vm.edit(null)}
          >
            New
          </Button>
        </div>
        <StatusLine className="routines-banner" id="routines-banner" hidden={!banner}>
          {banner}
        </StatusLine>
        <RoutineEditor editor={vm.editor} />
        <ul className="routines-list" id="routines-list" aria-label="Saved routines">
          {list.map((routine) => (
            <RoutineCard key={routine.id} vm={vm} state={state} routine={routine} />
          ))}
          {!list.length && (
            <li className="routines-empty">
              <strong>{ROUTINES_TEXT.empty}</strong>
              <span>{ROUTINES_TEXT.emptyHint}</span>
            </li>
          )}
        </ul>
      </div>
    </section>
  );
}
