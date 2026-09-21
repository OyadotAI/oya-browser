/** Fields the auth and admission middleware attach to a request. */
import type { User } from '@supabase/supabase-js';

/** Who is calling, as resolved from their credential. */
export interface Principal {
  /** The key the caller acts under: the API key itself, or its project's key. */
  key: string;
  /** What the credential may do. */
  role: 'administrator' | 'operator' | 'viewer' | 'browser';
  /** The session a browser-scoped credential is bound to. */
  sessionId?: string;
  [extra: string]: unknown;
}

declare global {
  namespace Express {
    /** Request fields set by this server's middleware. */
    interface Request {
      /** The bearer credential exactly as presented (authMiddleware, cluster hop). */
      authToken?: string;
      /** Resolved caller, set by the auth middleware. */
      principal?: Principal;
      /** Supabase user, set by userAuthMiddleware. */
      user?: User;
      /** Reservation from control admission, when the route admits a session. */
      controlSession?: any;
      /** Policies of the session being replaced, inherited by its replacement. */
      recoveryPolicies?: any[];
      /** Unparsed body, kept for Slack signature checks. */
      rawBody?: Buffer;
    }
    /** Response fields set by this server. */
    interface Response {
      /** A live-view stream keeps its viewer's credential so the registry can re-check access. */
      authToken?: string;
    }
  }
}
