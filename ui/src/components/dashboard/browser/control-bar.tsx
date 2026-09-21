/**
 * Who drives the browser right now, and the button that changes it.
 */
import { buttonLabel, modeText } from './control';

/** The mode and how to change it. */
interface Props {
  /** 'agent', 'human' or 'paused'. */
  mode: string;
  /** Takes, releases or resumes control, by mode. */
  onToggle: () => void;
}

/** "Agent control · take control to drive" and "Take control", and so on per mode. */
export default function ControlBar({ mode, onToggle }: Props) {
  return (
    <div className="flex items-center justify-between gap-2 text-xs">
      <span className="text-text-dim">{modeText(mode)}</span>
      <button className="btn-secondary text-xs" onClick={onToggle}>
        {buttonLabel(mode)}
      </button>
    </div>
  );
}
