/** Browser-owned tab notifications are independent of renderer IPC and external protocol transports. */
export class WindowTabEvents {
  /** Subscribers see only a change signal; each consumer applies its own authorization and target filtering. */
  private readonly listeners = new Set<() => void>();
  /** Subscribe without patching shell methods or installing page listeners. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  /** A broken observer must not interrupt tab creation, protection or normal browser interaction. */
  changed(): void {
    for (const listener of [...this.listeners]) {
      try {
        listener();
      } catch {
        console.warn('[tabs] Observer failed');
      }
    }
  }
}
