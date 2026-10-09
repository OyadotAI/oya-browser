/** Native DownloadItems remain scoped to owned contexts; default-profile download policy is never replaced. */
import { randomUUID } from 'node:crypto';
import type { DownloadItem, Session, WebContents, Event } from 'electron';
import { DOWNLOAD_LIMIT } from './constants.ts';
import { downloadDirectory, reserveDownload } from './repository.ts';
/** Only explicit protected pages can start downloads an agent controls. */
export interface DownloadDependencies {
  /** Rechecked for every start, update and terminal event. */
  allowed(contents: WebContents): boolean;
  /** Resolve the exact protected page identity, never the selected tab. */
  target(contents: WebContents): string;
}
/** Browser events are supplied by an authenticated browser-endpoint connection. */
type Emit = (method: string, params: Record<string, unknown>) => void;
/** Session policy has a single listener; changing it does not replace other session observers. */
interface Policy {
  /** No destination means deny. */
  directory?: string;
  /** Event delivery is explicitly requested. */
  emit?: Emit;
  /** Exact listener removed during context teardown. */
  listener(event: Event, item: DownloadItem, contents: WebContents): void;
}
/** One active item with its immutable ownership and safe destination. */
interface Transfer {
  /** Real item, not a re-fetched response. */
  item: DownloadItem;
  /** Real originating page. */
  contents: WebContents;
  /** Exact context session. */
  session: Session;
  /** Destination selected before native transfer. */
  path: string;
}
/** One connection's native download policies and cancellable transfers. */
export class NativeDownloads {
  /** Per-context policy, never the user's default session. */
  private readonly policies = new Map<Session, Policy>();
  /** Active IDs are unguessable and cannot address another socket's item. */
  private readonly items = new Map<string, Transfer>();
  /** Authorization and target resolution callbacks. */
  private readonly deps: DownloadDependencies;
  /** Construct without changing any session globally. */
  constructor(deps: DownloadDependencies) {
    this.deps = deps;
  }
  /** Private contexts deny downloads until an explicit owned policy is installed. */
  install(session: Session): void {
    if (this.policies.has(session)) throw Error('Download policy already installed');
    const policy: Policy = { listener: (_event, item, contents) => this.start(session, item, contents) };
    this.policies.set(session, policy);
    session.on('will-download', policy.listener);
  }
  /** Only named-GUID saving is supported; ordinary filename/overwrite semantics are not guessed. */
  set(session: Session, params: Record<string, unknown>, emit: Emit): void {
    const policy = this.policies.get(session);
    if (!policy) throw Error('Download context is unavailable');
    const directory = params.behavior === 'allowAndName' ? downloadDirectory(params.downloadPath as string) : undefined;
    policy.directory = directory;
    policy.emit = params.eventsEnabled === true ? emit : undefined;
  }
  /** Invalid origins, human-owned control and capacity overflow cancel before any save dialog opens. */
  private start(session: Session, item: DownloadItem, contents: WebContents): void {
    const policy = this.policies.get(session);
    if (!policy?.directory || !this.canStart(session, contents)) return item.cancel();
    try {
      this.save(session, item, contents, policy);
    } catch {
      item.cancel();
    }
  }
  /** Guard every initiating page before choosing a disk destination. */
  private canStart(session: Session, contents: WebContents): boolean {
    return (
      !!contents && contents.session === session && this.deps.allowed(contents) && this.items.size < DOWNLOAD_LIMIT
    );
  }
  /** Reserve the native destination and install progress observers before publishing the transfer. */
  private save(session: Session, item: DownloadItem, contents: WebContents, policy: Policy): void {
    const guid = randomUUID(),
      path = reserveDownload(policy.directory!, guid);
    item.setSavePath(path);
    const transfer = { item, contents, session, path };
    this.items.set(guid, transfer);
    item.on('updated', () => this.progress(guid, 'inProgress'));
    item.once('done', (_event, state) => this.finished(guid, state));
    policy.emit?.('Browser.downloadWillBegin', downloadStart(this.deps.target(contents), guid, item));
  }
  /** Human takeover cancels a transfer instead of leaking its progress or continuing an unowned write. */
  private progress(guid: string, state: string): void {
    const transfer = this.items.get(guid);
    if (!transfer) return;
    if (!this.deps.allowed(transfer.contents)) return transfer.item.cancel();
    this.policies.get(transfer.session)?.emit?.('Browser.downloadProgress', downloadProgress(transfer, guid, state));
  }
  /** Native terminal states map to the standard completed/canceled terminal states. */
  private finished(guid: string, state: string): void {
    this.progress(guid, state === 'completed' ? 'completed' : 'canceled');
    this.items.delete(guid);
  }
  /** Cancellation requires both the connection and exact context owning the transfer. */
  cancel(session: Session, guid: string): void {
    const transfer = this.items.get(guid);
    if (!transfer || transfer.session !== session) throw Error('Unknown or foreign native download');
    transfer.item.cancel();
  }
  /** Context disposal cancels only its transfers and removes only its listener. */
  remove(session: Session): void {
    for (const [guid, transfer] of this.items)
      if (transfer.session === session) {
        this.items.delete(guid);
        transfer.item.cancel();
      }
    const policy = this.policies.get(session);
    if (policy) session.off('will-download', policy.listener);
    this.policies.delete(session);
  }
}

/** Source metadata comes from the native item, never a caller-supplied filename. */
function downloadStart(frameId: string, guid: string, item: DownloadItem): Record<string, unknown> {
  return { frameId, guid, url: item.getURL(), suggestedFilename: item.getFilename() };
}
/** Report actual native byte counts and only the chosen GUID path on completion. */
function downloadProgress({ item, path }: Transfer, guid: string, state: string): Record<string, unknown> {
  return {
    guid,
    totalBytes: item.getTotalBytes(),
    receivedBytes: item.getReceivedBytes(),
    state,
    ...(state === 'completed' ? { filePath: path } : {}),
  };
}
