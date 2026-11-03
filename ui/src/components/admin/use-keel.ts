/** Keel reuses credential-scoped feed ownership while keeping its own loading and refresh lifecycle. */
import { adminKeel } from '@/lib/api';
import { useAdminFeed } from './use-admin-feed';
import type { KeelSnapshot } from './keel-types';

/** Only the selected Keel tab owns this independently retryable request. */
export function useKeel(token: string | null) {
  const { value, ...state } = useAdminFeed<KeelSnapshot>(token, adminKeel);
  return { ...state, snapshot: value };
}
