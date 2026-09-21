/**
 * The contract between the connection and its message handlers: what a
 * handler may use of the connection, and the handler shape itself.
 */

/** A persona, as handlers refer to it. */
export interface PersonaRef {
  /** Persona id: the key to its cookie jar and storage. */
  id: string;
}

/** The parts of a browser connection a handler works with. */
export interface Connection {
  /** The authenticated browser. */
  browserId: string;
  /** The key it authenticated as. */
  apiKey: string;
  /** The persona it runs as. */
  persona: PersonaRef;
  /** Whether it uses the metered residential gateway. */
  residentialProxy: boolean;
  /** Desktop-held command slots, by token. */
  localCommands: Map<string, () => Promise<void>>;
  /** A control handoff is in progress. */
  changingControl: boolean;
  /** Records a sign of life. */
  heard(): void;
  /** Sends a message to the browser. */
  send(message: object): void;
  /** Whether the socket can still be written to. */
  isOpen(): boolean;
  /** Whether this socket is still the browser's registered one. */
  isCurrent(): boolean;
}

/** Handles one message type for one connection. */
export type Handler = (conn: Connection, msg: any) => void | Promise<void>;
