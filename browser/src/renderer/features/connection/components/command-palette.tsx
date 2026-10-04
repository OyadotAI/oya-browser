/**
 * The command palette's page of the shell dialog (index.html's
 * #commands-section): the search and the matching commands. Down moves from
 * the search into the list, Up and Down wrap around it, Enter runs the first.
 */
import type { KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { PaletteViewModel } from '../view-models/palette-view-model.ts';
import type { PageProps } from '../model/models.ts';

/** The palette's command buttons, in order. */
const commandButtons = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('.command')];

/** Down moves into the list; Enter runs the first match. */
function searchKey(event: KeyboardEvent, vm: PaletteViewModel): void {
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    commandButtons()[0]?.focus();
  }
  if (event.key === 'Enter') void vm.runFirst();
}

/** Up and Down move between commands, wrapping around. */
function listKey(event: KeyboardEvent): void {
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
  event.preventDefault();
  const buttons = commandButtons();
  const index = buttons.indexOf(document.activeElement as HTMLElement);
  const step = event.key === 'ArrowDown' ? 1 : buttons.length - 1;
  buttons[(index + step) % buttons.length]?.focus();
}

/** The search and the commands matching it. */
export function CommandPalette({ vm, hidden }: PageProps<PaletteViewModel>) {
  const { query } = useViewModel(vm);
  const matches = vm.matches;
  return (
    <section id="commands-section" hidden={hidden}>
      <input
        id="command-search"
        aria-label="Search commands"
        placeholder="Search commands…"
        autoComplete="off"
        value={query}
        onChange={(event) => vm.search(event.target.value)}
        onKeyDown={(event) => searchKey(event, vm)}
      />
      <div id="command-list" onKeyDown={listKey}>
        {matches.map((command) => (
          <button key={command.id} className="command" onClick={() => void vm.run(command.id)}>
            <span>{command.label}</span>
            <kbd>{command.shortcut}</kbd>
          </button>
        ))}
        {matches.length ? null : <p>No matching commands.</p>}
      </div>
    </section>
  );
}
