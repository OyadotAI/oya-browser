/**
 * The fleet console. A thousand browsers in a table with their health, one
 * of them open on the right, and a keyboard to move between them. The state
 * lives in _console/use-console.ts; the parts render from it.
 */
'use client';

import Header from '@/components/dashboard/header';
import Onboarding from '@/components/dashboard/onboarding';
import { ConsoleBody } from './_console/console-body';
import { ConsoleDialogs } from './_console/console-dialogs';
import { useConsole } from './_console/use-console';

/** The console page. */
export default function DashboardPage() {
  const c = useConsole();
  const { view, patch, apiKey, config } = c;
  const onboarding = view.showOnboarding && config && apiKey;
  return (
    <div className="flex h-dvh flex-col overflow-hidden bg-bg">
      <Header apiKey={apiKey} setApiKey={c.openProject} onOpenSettings={() => patch({ showSettings: true })} />
      {onboarding ? (
        <Onboarding
          apiKey={apiKey}
          config={config}
          personas={c.personas}
          browsers={c.browsers}
          onDone={() => (patch({ showOnboarding: false }), c.fetchConfig())}
        />
      ) : (
        <ConsoleBody c={c} />
      )}
      <ConsoleDialogs c={c} />
    </div>
  );
}
