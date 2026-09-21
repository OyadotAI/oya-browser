/**
 * Resolving the persona a request asks for, answering the request when it
 * cannot be resolved.
 */
import { container } from '../../../app/container.ts';

/** Not yet layered: reads the persona service from the composition root. */
const { personas } = container;

/** The persona as `{ persona }`, or null once the resolver's error (status else `fallback`) is answered. */
export function resolvePersona(res, key: string, requested, fallback: number) {
  try {
    return { persona: personas.resolve(key, requested) };
  } catch (err) {
    res.status(err.status || fallback).json({ error: err.message });
    return null;
  }
}
