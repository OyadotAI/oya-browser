/**
 * The project's playbooks for the editor's "Does" choice and the values a routine
 * sets, loaded once when the editor opens, with why they could not be loaded.
 */
import { useEffect, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { listPlaybooks } from '../playbooks/api';
import type { PlaybookInfo } from '../playbooks/types';

/** The playbooks and their names (empty until loaded), and the load error. */
export function usePlaybookNames(apiKey: string) {
  const [list, setList] = useState<PlaybookInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    listPlaybooks(apiKey)
      .then((answer) => setList(answer.playbooks))
      .catch((err) => setError(errorMessage(err)));
  }, [apiKey]);
  return { list, names: list.map((p) => p.name), error };
}
