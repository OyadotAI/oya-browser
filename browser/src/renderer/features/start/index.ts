/**
 * The start page's entry: the page the composition root mounts where a new
 * tab's page would be, its ViewModel, and the `StartAsker` the root satisfies
 * with the Ask feature (`ask` → AskViewModel.ask, `busy` → its state.sending).
 */
export { StartPage, type StartPageProps } from './components/start-page.tsx';
export { StartViewModel, type StartAsker, type StartDeps, type StartState } from './view-models/start-view-model.ts';
