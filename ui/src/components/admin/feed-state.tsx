/** Consistent snapshot freshness and recovery controls keep each administrative source understandable. */
import type { ReactNode } from 'react';
import { utcTime } from './model';

/** Refresh ownership is shared by independent feeds without coupling their failures. */
export interface FeedState {
  /** A request is pending. */ loading: boolean;
  /** Most recent request failure. */ error: string;
  /** Starts this source's request only. */ refresh: () => void;
}

/** Every tab states what it measures and when its snapshot was read. */
export function SnapshotHeading({
  title,
  scope,
  updated,
  feed,
}: {
  /** Selected administrative domain. */ title: string;
  /** Population and time scope. */ scope: string;
  /** Last successful snapshot time. */ updated?: string;
  /** Independent source state. */ feed: FeedState;
}) {
  return (
    <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-start">
      <div className="space-y-1">
        <h2 className="font-display text-xl text-text">{title}</h2>
        <p className="text-xs text-text-muted">{scope}</p>
        <p role="status" className="text-xs text-text-dim">
          {updated ? `Updated ${utcTime(updated)}` : 'No snapshot loaded yet.'}
        </p>
      </div>
      <button
        type="button"
        onClick={feed.refresh}
        disabled={feed.loading}
        aria-label={`Refresh ${title}`}
        className="min-h-11 rounded-md border border-border px-4 text-xs text-text hover:bg-bg-card focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50"
      >
        {feed.loading ? 'Refreshing…' : feed.error ? 'Retry' : 'Refresh'}
      </button>
    </div>
  );
}

/** Ordinary refresh errors retain and explicitly label the last successful data. */
export function FeedNotice({
  feed,
  available,
  name,
}: {
  /** Independent source status. */ feed: FeedState;
  /** A previous successful snapshot is still displayed. */ available: boolean;
  /** The source being read. */ name: string;
}) {
  if (feed.error)
    return (
      <p role="alert" className="text-sm text-red">
        {available ? 'Couldn’t refresh. Showing the previous snapshot. ' : `${name} could not be loaded. `}
        {feed.error}
      </p>
    );
  if (feed.loading)
    return (
      <p role="status" className="text-xs text-text-muted">
        {available ? `Refreshing ${name}…` : `Loading ${name}…`}
      </p>
    );
  return null;
}

/** Content stays mounted during refresh so forms and selections retain their state. */
export function FeedBody({
  feed,
  available,
  name,
  children,
}: {
  /** Independent source status. */ feed: FeedState;
  /** Whether a successful snapshot exists. */ available: boolean;
  /** Source label for loading and errors. */ name: string;
  /** Successful snapshot content. */ children: ReactNode;
}) {
  return (
    <>
      <FeedNotice feed={feed} available={available} name={name} />
      {available && children}
    </>
  );
}
