/** MIME database lookup used only by the host-side workflow file repository. */
declare module 'mime-types' {
  /** Stable file-extension MIME resolution. */
  const mime: { /** Return a known MIME type or false for unknown suffixes. */ lookup(path: string): string | false };
  export default mime;
}
