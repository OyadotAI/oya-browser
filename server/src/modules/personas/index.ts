/**
 * The personas module's public surface: the domain (service, storage, model).
 * Its HTTP adapter, routes.ts, is mounted by app/api.ts, so building the
 * service never pulls in the web layer.
 */
export { PersonaService, type PersonaDeps, type CreatePersona, type UpdatePersona } from './service.ts';
export { type PersonaRepository, PersonaTable } from './repository.ts';
export { type Persona, type PersonaPrefs, type PersonaProxy, describeProfile } from './model.ts';
