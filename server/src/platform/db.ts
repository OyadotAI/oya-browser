/**
 * Supabase Auth, for email, Google and GitHub sign-in. Sign-in only: every
 * table lives behind platform/storage, whichever driver is configured.
 */

import { createClient } from '@supabase/supabase-js';

const supabaseUrl = process.env.SUPABASE_URL || '';
const supabaseKey = process.env.SUPABASE_SERVICE_KEY || '';

/** Supabase Auth client (or null if not configured). Never used for data. */
export const dbAuth = supabaseUrl && supabaseKey ? createClient(supabaseUrl, supabaseKey) : null;

console.log(`[auth] sign-in: ${dbAuth ? 'Supabase Auth' : 'API keys only'}`);
