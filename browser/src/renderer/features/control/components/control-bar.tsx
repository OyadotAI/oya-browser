/**
 * The toolbar's control status (index.html's #control-status): who drives,
 * Take control / Release, and Return to agent after a pause. It also installs
 * the watch-only guard and gate over the toolbar's page actions, which other
 * features render (see use-control-guard.ts).
 */
import { useViewModel } from '../../../hooks/index.ts';
import { controlLook, type ControlViewModel } from '../view-models/control-view-model.ts';
import { useControlGate, useControlGuard } from '../hooks/use-control-guard.ts';
import './control-bar.css';

/** What the control bar is given. */
export interface ControlBarProps {
  /** The control ViewModel. */
  vm: ControlViewModel;
}

/** The control status and its buttons. */
export function ControlBar({ vm }: ControlBarProps) {
  const state = useViewModel(vm);
  useControlGuard(vm);
  useControlGate(state);
  const look = controlLook(state);
  return (
    <div className="control-status" id="control-status" data-mode={look.mode} title={look.title}>
      <span className="control-orbit" aria-hidden="true"></span>
      <span id="control-label" role="status">
        {look.label}
      </span>
      <button
        id="control-action"
        hidden={look.action.hidden}
        disabled={look.action.disabled}
        onClick={() => void vm.toggle()}
      >
        {look.action.text}
      </button>
      <button
        id="control-resume"
        hidden={look.resume.hidden}
        disabled={look.resume.disabled}
        onClick={() => void vm.resume()}
      >
        Return to agent
      </button>
    </div>
  );
}
