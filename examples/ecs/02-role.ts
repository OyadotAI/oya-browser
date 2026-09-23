// Run Oya Cloud browsers on ECS through a role Oya assumes: no long-lived AWS keys leave your account.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
// Trust Oya's AWS account in the role, with this as sts:ExternalId. It is yours alone and cannot be set.
const { ecs } = await oya.config.get();
console.log('sts:ExternalId for the trust policy:', ecs.externalId);

await oya.config.set({
  sandbox_runtime: 'ecs',
  ecs: {
    cluster: process.env.ECS_CLUSTER!,
    taskDefinition: process.env.ECS_TASK_DEFINITION!,
    subnets: process.env.ECS_SUBNETS!.split(','),
    assignPublicIp: true,
    region: process.env.AWS_REGION!,
    auth: { type: 'role', roleArn: process.env.ECS_ROLE_ARN! }, // e.g. arn:aws:iam::123456789012:role/oya-browsers
  },
});

await using browser = await oya.browser.start({ provider: 'oya-cloud' });
await browser.goto('https://example.com');
console.log('Running on ECS at', await browser.url());
