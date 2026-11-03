/** The independent Keel admin response mirrors only its documented anonymous metrics. */

/** Measured totals and their independent report coverage. */
export interface KeelUsage {
  /** Eligible completed-day reports, excluding presence-only markers. */ reports: number;
  /** Reports measuring runtime, including measured zero. */ runtime_reports: number;
  /** Reports measuring projects, including an empty project set. */ project_reports: number;
  /** Supervised concurrent agent milliseconds; null is unavailable. */ runtime_ms: number | null;
  /** Distinct installation/project pairs; null is unavailable. */ projects: number | null;
  /** Reported effectful steps; older releases can undercount. */ steps: number;
}

/** One completed UTC day; missing days remain unavailable. */
export interface KeelDay extends KeelUsage {
  /** The usage date, not its network receipt date. */ day: string;
}

/** Current anonymous installation metadata and its measured usage. */
export interface KeelInstall extends KeelUsage {
  /** Random installation ID. */ install_id: string;
  /** Latest reported release. */ version: string;
  /** Latest reported platform. */ os: string;
  /** Latest reported opaque license ID, never a complete signed key. */ license_id: string | null;
  /** Earliest original server receipt. */ first_seen: string;
  /** Most recent server receipt. */ last_seen: string;
}

/** A reported identifier is not a verified issued-key inventory. */
export interface KeelLicense {
  /** Client-reported opaque identifier. */ license_id: string;
  /** Recently reporting installations currently naming this ID. */ installs: number;
  /** Most recent receipt among those installations. */ last_seen: string;
}

/** Bounded snapshot behind the staff authentication and MFA guard. */
export interface KeelSnapshot {
  /** When this snapshot was generated. */ generated_at: string;
  /** Latest server receipt, or null before any report. */ latest_report: string | null;
  /** Explicit completed-day and reporting-presence windows. */ period: {
    /** First completed UTC date. */ from: string;
    /** Last completed UTC date. */ to: string;
    /** Completed days shown. */ days: number;
    /** Receipt-based activity window. */ active_days: number;
  };
  /** Window-wide measurements and coverage. */ usage: KeelUsage;
  /** Oldest day first, with all missing dates present. */ days: KeelDay[];
  /** Full counts with a bounded most-recent list. */ installs: {
    /** Seen at any time. */ total: number;
    /** Reporting in the presence window. */ active: number;
    /** Most recent installations, at most fifty. */ list: KeelInstall[];
  };
  /** Current reported IDs from recently active installations. */ licenses: {
    /** Full distinct count. */ total: number;
    /** Bounded most-recent groups. */ list: KeelLicense[];
  };
}
