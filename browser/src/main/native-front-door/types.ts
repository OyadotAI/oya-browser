/** Narrow native capabilities available to the external CDP compatibility boundary. */
export interface NativeTarget {
  /** Opaque browser-owned identity, never supplied by a page or reused after closure. */
  targetId: string;
  /** Only application-owned page tabs are exposed. */
  type: 'page';
  /** Present only for this connection's explicitly private session. */
  browserContextId?: string;
  /** Current document title. */
  title: string;
  /** Current document address. */
  url: string;
}
/** Native observer cleanup may support atomic updates without dropping already-held requests. */
export type NativeSubscription = (() => void) & {
  /** Only implemented native policy updates are exposed. */
  update?: (params: Record<string, unknown>) => void;
};
/** No debugger, upstream URL or protocol sender is a backend capability. */
export interface NativeBackend {
  /** Create connection-local node handles and subscriptions, sharing only native target identities. */
  connection?(): NativeBackend;
  /** Release connection-local native resources. */
  dispose?(): void;
  /** Observe a native event domain on one exact target; caller owns the returned unsubscribe. */
  subscribe?(
    target: string,
    domain: string,
    emit: (method: string, params: Record<string, unknown>) => void,
    params?: Record<string, unknown>,
  ): NativeSubscription;
  /** Subscribe to authorized native tab changes; never inspect renderer IPC or proxy debugging events. */
  watchTargets?(changed: () => void): () => void;
  /** Select and show an exact protected native tab. */
  activate?(target: string): void;
  /** Resolve an already-held native callback, with exact ownership and control checks. */
  resolveRequest?(target: string, id: string, cancel: boolean, reason?: string): void;
  /** List only tabs authorized for this endpoint. */
  targets(): NativeTarget[];
  /** Open a tab through normal protection and egress policy. */
  open(url: string, context?: string): Promise<string>;
  /** Native context/download operations scoped to this connection. */
  manage?(
    action: string,
    params: Record<string, unknown>,
    emit: (method: string, params: Record<string, unknown>) => void,
  ): Promise<object>;
  /** Close an owned, authorized tab. */
  close(target: string): Promise<void>;
  /** Execute a supported native operation on this exact target. */
  execute(target: string, action: string, params: Record<string, unknown>): Promise<unknown>;
}
/** An explicitly authorized local listener, independent of any debugging endpoint. */
export interface NativeDoorOptions {
  /** Bind only to loopback; zero requests an ephemeral port. */
  port: number;
  /** Required bearer credential; never placed in URLs or discovery responses. */
  token: string;
  /** Browser-owned operations only. */
  backend: NativeBackend;
  /** Acquire the existing human/agent command gate. */
  beginCommand(): Promise<() => void> | (() => void);
  /** Maintain existing local-client ownership semantics. */
  clientChanged(delta: number): void;
}
/** Flat CDP command accepted after shape validation. */
export interface NativeCommand {
  /** Reply correlation id. */
  id: number;
  /** Supported CDP or documented Oya extension method. */
  method: string;
  /** Per-connection attached target identity. */
  sessionId?: string;
  /** Method arguments. */
  params: Record<string, unknown>;
}
