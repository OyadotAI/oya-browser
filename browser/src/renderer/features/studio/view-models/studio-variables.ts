/**
 * The studio's variable intents: rename, mark secret (a secret loses its
 * default), set a default, remove. Each builds on the variables as they are
 * when its command's turn comes, so two quick edits both land.
 */
import type { StudioViewModel } from './studio-view-model.ts';
import type { VariableConfig, Variables } from '../model/types.ts';

/** The variables. */
export class StudioVariables {
  /** The studio whose draft holds them. */
  private readonly studio: StudioViewModel;

  /** Edits the variables of `studio`'s draft. */
  constructor(studio: StudioViewModel) {
    this.studio = studio;
  }

  /** Replaces the variables with what `next` makes of them as they are when its turn comes. */
  private save(next: (variables: Variables) => Variables): Promise<unknown> {
    return this.studio.command(() => ({
      type: 'variables',
      variables: next(this.studio.workspace?.draft.variables ?? {}),
    }));
  }

  /** Changes one variable's settings, on top of the variables as they are then. */
  private change(name: string, changes: VariableConfig): Promise<unknown> {
    return this.save((variables) => ({ ...variables, [name]: { ...variables[name], ...changes } }));
  }

  /** Renames a variable everywhere it is used. */
  rename(name: string, nextName: string): Promise<unknown> {
    return this.studio.command({ type: 'rename-variable', name, nextName });
  }

  /** Sets a variable's default value. */
  setDefault(name: string, value: string): Promise<unknown> {
    return this.change(name, { default: value });
  }

  /** Marks a variable secret (dropping its default) or not. */
  setSecret(name: string, secret: boolean): Promise<unknown> {
    return this.change(name, { secret, ...(secret ? { default: undefined } : {}) });
  }

  /** Removes a variable. */
  remove(name: string): Promise<unknown> {
    return this.save((variables) => Object.fromEntries(Object.entries(variables).filter(([key]) => key !== name)));
  }
}
