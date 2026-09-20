/**
 * The landing page's fixed content: the repository link and the portal example.
 */

/** The public repository. */
export const repository = 'https://github.com/OyadotAI/oya-browser';
/** The example shown and copied in the developers section. */
export const portalExample = `import { Oya } from "@oya-ai/browser";

const oya = new Oya(); // Set OYA_API_KEY.
const { PORTAL_URL, PORTAL_USERNAME, PORTAL_PASSWORD } = process.env;
if (!PORTAL_URL || !PORTAL_USERNAME || !PORTAL_PASSWORD) {
  throw new Error("Set your test portal URL and credentials.");
}
const secrets = { username: PORTAL_USERNAME, password: PORTAL_PASSWORD };
const playbookName = "portal-request-review";

// First run: let the agent do the task, then store its playbook.
{
  await using browser = await oya.browser.start();
  await browser.goto(PORTAL_URL);
  await browser.ask(
    "If needed, log in with {{username}} and {{password}}. " +
    "Open New Request for {{customerName}}. Stop before submitting.",
    { data: { customerName: "Alex Example" }, secrets },
  );
  await browser.toPlaybook(playbookName);
} // The first browser stops here; the playbook stays saved.

// Later: run the saved playbook on a new browser with new inputs.
await using replay = await oya.browser.start();
await replay.goto(PORTAL_URL);
await replay.play(playbookName, {
  customerName: "Sam Example",
  ...secrets, // Replay remembers which variables are secret.
});`;
