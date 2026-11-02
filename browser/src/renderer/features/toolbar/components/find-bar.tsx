/**
 * The find bar (Cmd/Ctrl+F): a field, the match count, previous and next, and
 * close, in one row under the toolbar that the main process makes room for.
 */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon, IconButton } from '../../../ui/index.ts';
import { commandShortcut } from '../../../../shared/shortcuts.ts';
import { FIND_QUERY_MAX_CHARS } from '../../../../shared/constants.ts';
import { findCount, type FindBarViewModel } from '../view-models/find-bar-view-model.ts';
import { TEXT } from '../model/constants.ts';
import './find-bar.css';

/** What the bar is given. */
export interface FindBarProps {
  /** The bar's ViewModel. */
  vm: FindBarViewModel;
  /** navigator.platform, for the buttons' shortcut hints. */
  platform: string;
}

/** A button's name with its shortcut, as "Next match (⌘ G)". */
const withKeys = (label: string, command: string, platform: string): string =>
  `${label} (${commandShortcut(command, platform)})`;

/** The field: takes focus and selects its text on each Cmd/Ctrl+F. */
function FindField({ vm }: Pick<FindBarProps, 'vm'>) {
  const { query, focusRequest, open } = useViewModel(vm);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (focusRequest && open) input.current?.select();
  }, [focusRequest, open]);
  return (
    <label className="find-field">
      <Icon name="search" />
      <input
        ref={input}
        type="text"
        value={query}
        maxLength={FIND_QUERY_MAX_CHARS}
        placeholder={TEXT.find}
        aria-label={TEXT.find}
        aria-describedby="find-count"
        onChange={(e) => vm.type(e.target.value)}
        onKeyDown={(e) =>
          vm.key({
            key: e.key,
            shiftKey: e.shiftKey,
            isComposing: e.nativeEvent.isComposing,
            preventDefault: () => e.preventDefault(),
          })
        }
      />
    </label>
  );
}

/** The find bar. */
export function FindBar({ vm, platform }: FindBarProps) {
  const state = useViewModel(vm);
  const none = !state.total;
  return (
    <div className="find-bar" id="find-bar" hidden={!state.open} role="search" aria-label={TEXT.find}>
      <FindField vm={vm} />
      <span className="find-count" id="find-count" role="status" aria-live="polite">
        {findCount(state)}
      </span>
      <IconButton
        className="find-step"
        icon="up"
        label={withKeys(TEXT.findPrevious, 'find-previous', platform)}
        disabled={none}
        onClick={() => vm.step(false)}
      />
      <IconButton
        className="find-step"
        icon="down"
        label={withKeys(TEXT.findNext, 'find-next', platform)}
        disabled={none}
        onClick={() => vm.step(true)}
      />
      <IconButton className="find-close" icon="close" label={TEXT.findClose} onClick={() => vm.close()} />
    </div>
  );
}
