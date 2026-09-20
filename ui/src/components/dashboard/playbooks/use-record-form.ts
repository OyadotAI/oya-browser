/**
 * What the person types into the recording dialog: the address to open, and
 * the name and description the playbook is saved under.
 */
import { useState } from 'react';

/** The dialog's text fields. */
export function useRecordForm() {
  const [url, setUrl] = useState('');
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  return { url, setUrl, name, setName, description, setDescription };
}

/** The form's fields and setters. */
export type RecordForm = ReturnType<typeof useRecordForm>;
