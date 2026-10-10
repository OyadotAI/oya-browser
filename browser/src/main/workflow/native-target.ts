/** Native workflow locator snapshots; DOM actions retain exact document-local node capabilities. */
/** Read-only inspection produced in the selected native frame. */
export interface TargetRead {
  /** Number of eligible matches. */ count: number;
  /** Normalized text. */ text?: string;
  /** Exact native field value. */ value?: string;
  /** Recorded tag/link identity still matches. */ recorded?: boolean;
  /** Element can receive editing operations. */ editable?: boolean;
}
