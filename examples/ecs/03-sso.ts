// Run Oya Cloud browsers on ECS with an IAM Identity Center (SSO) sign-in instead of access keys.
// Get a token: `aws sso login --profile <yours>`, then
//   jq -r 'select(.accessToken) | .accessToken' ~/.aws/sso/cache/*.json
// It expires (1 to 12 hours); starts then fail with sso_token_expired until you set a fresh one.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await oya.config.set({
  sandbox_runtime: 'ecs',
  ecs: {
    cluster: process.env.ECS_CLUSTER!,
    taskDefinition: process.env.ECS_TASK_DEFINITION!,
    subnets: process.env.ECS_SUBNETS!.split(','),
    assignPublicIp: true,
    region: process.env.AWS_REGION!,
    auth: {
      type: 'sso',
      accessToken: process.env.AWS_SSO_ACCESS_TOKEN!,
      accountId: process.env.AWS_ACCOUNT_ID!, // the 12-digit account the permission set is in
      roleName: process.env.AWS_SSO_ROLE_NAME!, // the permission set's role name, e.g. PowerUserAccess
    },
  },
});

await using browser = await oya.browser.start({ provider: 'oya-cloud' });
await browser.goto('https://example.com');
console.log('Running on ECS at', await browser.url());
