/**
 * The Groups dialog's fixed words and the name length the server allows.
 */

/** The longest group name the server takes (GROUP_NAME_MAX there). */
export const GROUP_NAME_MAX = 64;

/** The words the Groups dialog and its editor show. */
export const GROUP_TEXT = {
  title: 'Groups',
  description:
    "Spread browser starts across several profiles. Your code passes the group's name; each start runs as one of its profiles.",
  loading: 'Loading groups…',
  failed: 'Could not load your groups',
  retry: 'Try again',
  done: 'Done',
  create: 'Create group',
  newGroup: 'New group',
  newTitle: 'New group',
  editTitle: (name: string) => `Edit ${name}`,
  name: 'Name',
  namePlaceholder: 'e.g. checkout-pool',
  nameHelp: "Letters, digits, dot, dash or underscore. Your code passes this name, so it can't be changed later.",
  noRename: "Names can't change. Delete the group and make a new one to rename it.",
  profiles: 'Profiles',
  howPicked:
    'Each start goes to the profile with the fewest running browsers, then the one used longest ago. Profiles at their cap are skipped.',
  pickOne: 'Pick at least one profile.',
  noEligible: "Make a profile first. The default profile can't be in a group.",
  cancel: 'Cancel',
  save: 'Save',
  saving: 'Saving…',
  confirmTitle: (name: string) => `Delete group ${name}?`,
  confirmBody: (name: string) =>
    `Starts that pass personaGroup '${name}' will fail until you make it again. Its profiles and their running browsers are not changed.`,
  confirmButton: 'Delete group',
} as const;
