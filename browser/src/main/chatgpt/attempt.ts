/** Bind provider setup to one workspace and cancel stale work before it can mutate another. */

/** Only these fields identify the destination of a provider setup. */
interface Destination {
  /** Control-plane socket address. */
  serverUrl: string;
  /** Workspace credential, possibly minted by this attempt. */
  apiKey: string;
}

/** A bounded provider operation owns its cancellation and expected destination. */
export class SetupAttempt {
  /** Cancellation shared by sign-up and model configuration requests. */
  readonly abort = new AbortController();
  /** Destination captured before leaving the app for authorization. */
  private expected: Destination;
  /** Reads the current destination without retaining an obsolete settings object. */
  private readonly destination: () => Destination;

  /** Takes the workspace snapshot before any asynchronous step. */
  constructor(destination: () => Destination) {
    this.destination = destination;
    this.expected = { ...destination() };
  }

  /** Rejects cancellation or a workspace change before any following side effect. */
  check(): void {
    this.abort.signal.throwIfAborted();
    const now = this.destination();
    if (now.serverUrl !== this.expected.serverUrl || now.apiKey !== this.expected.apiKey)
      throw Error('Your workspace changed. Start connecting again.');
  }

  /** Permits only this attempt's newly created credential to replace the original empty one. */
  adoptKey(apiKey: string): void {
    this.check();
    this.expected.apiKey = apiKey;
  }
}
