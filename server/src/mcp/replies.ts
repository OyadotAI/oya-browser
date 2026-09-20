/**
 * The shapes an MCP tool answers with, and a way to run work that may throw
 * without a try/catch at every call.
 */

/** A text reply. */
export const text = (t: string) => ({ content: [{ type: 'text' as const, text: t }] });

/** An error reply the model sees as a failed tool call. */
export const fail = (t: string) => ({ content: [{ type: 'text' as const, text: `Error: ${t}` }], isError: true });

/** How some work ended: its value, or what it threw. */
type Attempt<T> =
  | {
      /** What the work returned. */
      value: T;
    }
  | {
      /** What the work threw. */
      error: any;
    };

/** The work's value as `{ value }`, or what it threw as `{ error }`. */
export async function attempt<T>(work: () => Promise<T>): Promise<Attempt<T>> {
  try {
    return { value: await work() };
  } catch (error) {
    return { error };
  }
}
