/**
 * An in-memory stand-in for the audit anchor bucket: it keeps uploaded
 * objects by path, refuses to overwrite one, and lists a folder's names, as a
 * Supabase storage bucket with upsert disabled does.
 */

/** A fake anchor bucket; `failWith` makes every call answer that storage error. */
export function anchorBucket(failWith: string | null = null) {
  const objects = new Map<string, string>();
  const error = () => (failWith ? { message: failWith } : null);
  return {
    objects,
    /** Stores a new object; an existing path is an error, never an overwrite. */
    async upload(path: string, body: string, options: { upsert: boolean }) {
      if (failWith) return { error: error() };
      if (objects.has(path) && !options.upsert) return { error: { message: 'The resource already exists' } };
      objects.set(path, body);
      return { data: { path }, error: null };
    },
    /** Names directly under `prefix`. */
    async list(prefix: string) {
      if (failWith) return { data: null, error: error() };
      const names = [...objects.keys()]
        .filter((p) => p.startsWith(`${prefix}/`))
        .map((p) => p.slice(prefix.length + 1));
      return { data: names.map((name) => ({ name })), error: null };
    },
  };
}
