/**
 * The Downloads button, which becomes the download chip while a download from
 * this window runs or has just ended: "Saved · report.pdf  Open  Show in
 * Finder". It sits in the toolbar row, so it never covers or freezes the page
 * and never takes focus; a polite live region beside it says what happened.
 */
import { useLayoutEffect, useRef, type KeyboardEvent, type RefObject } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import type { DownloadChipViewModel, DownloadChipState } from '../view-models/download-chip-view-model.ts';
import { DOWNLOAD_TEXT, PERCENT } from '../model/constants.ts';
import type { DownloadAction } from '../../../../shared/ipc.ts';
import { DownloadRing } from './download-ring.tsx';
import './download-chip.css';

/** What the chip is given. */
export interface DownloadChipProps {
  /** The chip's ViewModel. */
  vm: DownloadChipViewModel;
}

/** What a part of the chip draws from. */
interface PartProps {
  /** The chip's state. */
  s: DownloadChipState;
}

/** The body: the state, a handle for focus, and what a click does. */
interface BodyProps extends PartProps {
  /** Where focus returns when the chip goes back to the plain button. */
  body: RefObject<HTMLButtonElement | null>;
  /** Opens the Downloads page. */
  onClick: () => void;
}

/** The actions: the state, and how to run one. */
interface ActionsProps extends PartProps {
  /** Opens the file or shows it in its folder. */
  act: (action: DownloadAction) => void;
}

/**
 * Keeps focus on the Downloads button when the part it was on goes away (Open or Show, as news from main puts
 * the chip back or a new download takes it over): it never falls to the page's body.
 */
function useFocusKept(vm: DownloadChipViewModel, s: DownloadChipState, body: RefObject<HTMLButtonElement | null>) {
  const box = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    if (vm.hasFocus() && !box.current?.contains(document.activeElement)) body.current?.focus();
  }, [vm, s.phase, s.id, body]);
  return box;
}

/** The Downloads button or chip, its Open and Show actions, its progress bar and its live region. */
export function DownloadChip({ vm }: DownloadChipProps) {
  const s = useViewModel(vm);
  const body = useRef<HTMLButtonElement>(null);
  const box = useFocusKept(vm, s, body);
  const settle = (done: boolean) => done && body.current?.focus();
  const act = (action: DownloadAction) => void vm.act(action, s.id).then(() => settle(vm.state.phase === 'idle'));
  const onKeyDown = (e: KeyboardEvent) => e.key === 'Escape' && settle(vm.escape());
  return (
    <span
      ref={box}
      className={`download-chip phase-${s.phase}`}
      onKeyDown={onKeyDown}
      onMouseEnter={() => vm.pointer(true)}
      onMouseLeave={() => vm.pointer(false)}
      onFocus={() => vm.focus(true)}
      onBlur={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && vm.focus(false)}
    >
      <ChipBody s={s} body={body} onClick={() => vm.showAll()} />
      {s.phase === 'saved' && <ChipActions s={s} act={act} />}
      {s.ring !== false && <ChipBar share={s.ring} />}
      <span className="sr-only" role="status" aria-live="polite">
        {s.announce}
      </span>
    </span>
  );
}

/** The button itself: the plain Downloads button when idle, else the ring or check, the status and the name. */
function ChipBody({ s, body, onClick }: BodyProps) {
  const idle = s.phase === 'idle';
  return (
    <button
      ref={body}
      className="library-button download-chip-body"
      id="downloads-open"
      title={s.title}
      aria-label={idle && s.ring === false ? undefined : s.label}
      onClick={onClick}
    >
      <ChipMark s={s} />
      {idle ? <span>{DOWNLOAD_TEXT.button}</span> : <ChipWords s={s} />}
    </button>
  );
}

/** The icon slot: the ring while anything runs, the check once saved, else the download icon. */
function ChipMark({ s }: PartProps) {
  if (s.ring !== false) return <DownloadRing share={s.ring} />;
  if (s.phase === 'saved')
    return (
      <span className="download-chip-mark">
        <Icon name="check" />
      </span>
    );
  return <Icon name="download" />;
}

/** "Saved · report.pdf": the status word, then the name with its extension kept whole. */
function ChipWords({ s }: PartProps) {
  return (
    <>
      <span className="download-chip-status">{s.status}</span>
      {s.named && (
        <span className="download-chip-name" aria-hidden="true">
          <span className="download-chip-sep"> · </span>
          <span className="download-chip-stem">{s.stem}</span>
          <span className="download-chip-ext">{s.ext}</span>
        </span>
      )}
      {s.meter && <span className="download-chip-meter">{s.meter}</span>}
    </>
  );
}

/** Open and Show in Finder (or folder), for a saved file. */
function ChipActions({ s, act }: ActionsProps) {
  const name = s.stem + s.ext;
  return (
    <>
      <button
        className="library-button download-chip-action"
        aria-label={DOWNLOAD_TEXT.openLabel(name)}
        onClick={() => act('open')}
      >
        {DOWNLOAD_TEXT.open}
      </button>
      <button
        className="library-button download-chip-action"
        aria-label={DOWNLOAD_TEXT.showLabel(name, s.mac)}
        onClick={() => act('reveal')}
      >
        <span className="download-chip-long">{DOWNLOAD_TEXT.show(s.mac)}</span>
        <span className="download-chip-short">{DOWNLOAD_TEXT.showShort}</span>
      </button>
    </>
  );
}

/** The meter's bar along the chip's bottom edge: how far what is running has got, or sliding when a size is unknown. */
function ChipBar({ share }: { /** The share arrived, or null when unknown. */ share: number | null }) {
  const width = share === null ? undefined : `${share * PERCENT}%`;
  return (
    <span className={`download-chip-bar${share === null ? ' sliding' : ''}`} aria-hidden="true">
      <span className="download-chip-fill" style={{ width }} />
    </span>
  );
}
