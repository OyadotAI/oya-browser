/** Session-only notifications delivered to the trusted browser shell. */
export interface BrowserNotification {
  /** Stable identity for dismissal. */
  id: string;
  /** Plain page text, never interpreted as markup. */
  message: string;
  /** Origin only; paths, credentials and query strings are not retained. */
  source: string;
  /** Arrival time in epoch milliseconds. */
  time: number;
  /** Whether the person has read this entry. */
  read: boolean;
}
