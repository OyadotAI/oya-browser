/** Three quiet, persistent collection buttons share one place in the browser chrome; Downloads turns into the download chip. */
import { Icon } from '../../../ui/index.ts';
import type { BookmarksBarViewModel } from '../view-models/bookmarks-bar-view-model.ts';
import { DownloadChip } from './download-chip.tsx';
import './library-buttons.css';
/** Searchable collections remain separate actions with explicit visible names. */
export function LibraryButtons({
  vm,
}: {
  /** Uses the same native collection commands in either chrome row. */
  vm: BookmarksBarViewModel;
}) {
  return (
    <div className="library-buttons" role="group" aria-label="Saved pages">
      <button
        className="library-button"
        id="bookmarks-open"
        title="Bookmarks (⌘/Ctrl Shift O)"
        onClick={() => vm.showAll()}
      >
        <Icon name="star" />
        <span>Bookmarks</span>
      </button>
      <button
        className="library-button"
        id="history-open"
        title="History (⌘/Ctrl Y)"
        onClick={() => vm.showAll('history')}
      >
        <Icon name="history" />
        <span>History</span>
      </button>
      <DownloadChip vm={vm.downloads} />
    </div>
  );
}
