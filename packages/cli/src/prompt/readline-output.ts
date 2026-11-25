/** Intercept Readline's direct cursor writes as well as its text echo without replacing its input reader. */

/** A display filter returns true after handling a write itself. */
type OutputFilter = (text: string) => boolean;

/** Satisfy Readline's optional completion callback for a write which was handled by the display filter. */
function completeWrite(args: unknown[]): void {
  const callback = args.at(-1);
  if (typeof callback === 'function') callback();
}

/** Forward ordinary output unchanged and intercept only writes selected by the active question. */
function filteredWrite(output: NodeJS.WriteStream, filter: OutputFilter): (...args: unknown[]) => boolean {
  return (...args) => {
    if (!filter(String(args[0]))) return Reflect.apply(output.write, output, args) as boolean;
    completeWrite(args);
    return true;
  };
}

/** Stream methods retain their real receiver; geometry and event properties remain those of the real stream. */
function streamProperty(
  output: NodeJS.WriteStream,
  key: string | symbol,
  write: (...args: unknown[]) => boolean,
): unknown {
  if (key === 'write') return write;
  const value: unknown = Reflect.get(output, key, output);
  return typeof value === 'function' ? value.bind(output) : value;
}

/** Readline keeps one reader and its terminal parsing, with an output boundary for plain setup display. */
export function readlineOutput(output: NodeJS.WriteStream, filter: OutputFilter): NodeJS.WriteStream {
  const write = filteredWrite(output, filter);
  return new Proxy(output, { get: (target, key) => streamProperty(target, key, write) });
}
