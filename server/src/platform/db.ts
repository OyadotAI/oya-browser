/**
 * Shared Supabase client for the oya_browser schema.
 */

import { createClient } from '@supabase/supabase-js';

const supabaseUrl = process.env.SUPABASE_URL || '';
const supabaseKey = process.env.SUPABASE_SERVICE_KEY || '';

/** Supabase client scoped to oya_browser schema (or null if not configured) */
export const db =
  supabaseUrl && supabaseKey ? createClient(supabaseUrl, supabaseKey, { db: { schema: 'oya_browser' } }) : null;

/** Supabase client using default (public) schema, for auth operations */
export const dbAuth = supabaseUrl && supabaseKey ? createClient(supabaseUrl, supabaseKey) : null;

// Naming what IS in use, not what is absent. "running without database" is
// alarming and wrong when SQLite or Postgres was chosen deliberately, the
// control plane always has durable storage, it just may not be this client.
if (db) console.log('[db] storage: Supabase');
else if (process.env.DATABASE_URL) console.log('[db] storage: Postgres (DATABASE_URL)');
else console.log(`[db] storage: SQLite in ${process.env.OYA_DATA_DIR || 'server/data'}, single replica`);
