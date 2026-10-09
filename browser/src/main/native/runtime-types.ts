/** Native runtime capability and context shapes, independent of the external protocol transport. */
import type { WebFrameMain } from 'electron';
/** Browser-process-only frame capability supplied by the patched engine. */
export interface RuntimeFrame extends WebFrameMain {
  /** Perform a fixed native V8 operation against one exact document and connection namespace. */
  _runOyaRuntime?: (owner: string, context: string, operation: string, params: object) => Promise<RuntimeReply>;
}
/** A native result distinguishes infrastructure failures from page exceptions. */
export interface RuntimeReply {
  /** Native operation failure; never reinterpret as page undefined. */
  error?: string;
  /** Native document identity, available only from context discovery. */
  context?: string;
  /** Serialized remote value descriptor. */
  result?: object;
  /** Native property descriptors, with accessors preserved rather than invoked. */
  properties?: object[];
  /** Page evaluation failure, independent from navigation/transport errors. */
  exception?: {
    /** Engine diagnostic without guessing source positions. */
    text: string;
    /** Connection-owned thrown value descriptor. */
    exception: object;
  };
}
/** One public main-world context is associated with one exact native document. */
export interface RuntimeContext {
  /** Monotonically allocated connection-local public context id. */
  id: number;
  /** Non-reusable public context identity. */
  uniqueId: string;
  /** Native document identity, never accepted directly from an external caller. */
  document: string;
  /** Identity shared with this connection’s native frame tree. */
  frameId: string;
  /** Browser-owned exact frame. */
  frame: RuntimeFrame;
  /** Exact protected target. */
  target: string;
}
/** Native runtime limits bound handles, frame wrappers, source and argument allocation. */
export const RUNTIME = {
  /** Maximum owned frame wrappers per external connection. */
  frames: 128,
  /** Maximum tracked native value identities across this connection. */
  handles: 262_144,
  /** Maximum UTF-16 source or group length admitted at the boundary. */
  source: 1_048_576,
  /** Maximum function arguments; engine enforces this independently. */
  arguments: 128,
} as const;
