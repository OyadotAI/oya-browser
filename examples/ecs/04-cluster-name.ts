// Point Oya at your ECS cluster by its name. The name is looked up in `region`.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await oya.config.set({
  sandbox_runtime: 'ecs',
  ecs: {
    cluster: 'browsers', // the cluster's name
    taskDefinition: process.env.ECS_TASK_DEFINITION!,
    subnets: process.env.ECS_SUBNETS!.split(','),
    assignPublicIp: true,
    region: 'us-east-1',
    auth: {
      type: 'iam',
      accessKeyId: process.env.AWS_ACCESS_KEY_ID!,
      secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY!,
    },
  },
});

await using browser = await oya.browser.start({ provider: 'oya-cloud' });
await browser.goto('https://example.com');
console.log('Running on ECS at', await browser.url());
