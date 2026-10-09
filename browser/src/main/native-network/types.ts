/** Native request metadata and authorization seams; no upstream protocol sender exists. */
import type { Session, WebContents, WebFrameMain } from 'electron';
/** Metadata delivered by the native session's request pipeline. */
export interface RequestDetails {
  /** Native request sequence, scoped to this exact session. */
  id: number;
  /** Exact requesting page where available. */
  webContents?: WebContents;
  /** Exact native frame; requests without a renderer are never attributed to a guessed page. */
  frame?: WebFrameMain | null;
  /** Requested URL. */
  url: string;
  /** Actual HTTP method. */
  method: string;
  /** Engine request category. */
  resourceType: string;
  /** Actual request headers, where the native phase supplies them. */
  requestHeaders?: Record<string, string>;
  /** Actual response headers. */
  responseHeaders?: Record<string, string[]>;
  /** Native response status. */
  statusCode?: number;
  /** Engine error text. */
  error?: string;
  /** Actual redirect destination. */
  redirectURL?: string;
}
/** Private engine capability returns original response bytes, not a second HTTP request. */
export type BodySession = Session & {
  /** Existing request hooks plus the explicit native response-pipe capability. */
  webRequest: Session['webRequest'] & {
    /** Explicit native error-selection support, checked before consuming a held request. */
    _supportsOyaRequestErrors?: () => boolean;
    /** Null revokes pending deliveries through the engine's listener epoch. */
    _setOyaBodyListener?: (listener: ((reply: BodyReply) => void) | null) => void;
  };
};
/** A terminal capture result; overflow is an error, never a truncated successful body. */
export interface BodyReply {
  /** Native request identity. */
  id: number;
  /** Complete response bytes encoded as base64. */
  body: string;
  /** Native overflow/cancellation/pipe error, or empty on successful EOF. */
  error: string;
}
/** Authenticated observer sink. */
export type NetworkSink = (method: string, params: Record<string, unknown>) => void;
/** Every asynchronous callback must preserve current control and exact target ownership. */
export interface NetworkDependencies {
  /** Human/agent control and protected target membership. */
  allowed(contents: WebContents): boolean;
  /** Policy rechecked before pause and before continuation. */
  allowedURL(url: string): boolean;
  /** Exact protected frame identity shared with Page.getFrameTree. */
  frameId(contents: WebContents, frame: WebFrameMain): string;
  /** Exact protected main page identity. */
  target(contents: WebContents): string;
}
/** Native pre-request continuation; URL/method/header overrides are intentionally absent. */
export type ResumeRequest = (decision: {
  /** Cancel only, never reroute around egress. */ cancel?: boolean;
  /** Validated native failure name; never a URL, body, or numeric code. */ _oyaErrorReason?: string;
}) => void;

/** A native policy observer owns cleanup and optional atomic policy replacement. */
export type NetworkObserver = (() => void) & {
  /** Replace validated filters without transferring ownership or releasing held requests. */
  update?: (params: Record<string, unknown>) => void;
};
