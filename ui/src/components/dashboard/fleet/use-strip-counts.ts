/**
 * The fleet strip's provider and persona counts, largest first, with the
 * personas past the first few summed into one "+N more".
 */
import { useMemo } from 'react';
import type { Fleet } from '../types';
import { TOP_PERSONAS } from './constants';
import { byCount } from './rows';

/** Provider and persona chips to show, from the latest fleet poll. */
export function useStripCounts(b: Fleet['browsers'] | undefined) {
  const providers = useMemo(() => byCount(b?.byProvider), [b]);
  const personas = useMemo(() => byCount(b?.byPersona), [b]);
  const topPersonas = personas.slice(0, TOP_PERSONAS);
  return { providers, topPersonas, restPersonas: personas.length - topPersonas.length };
}
