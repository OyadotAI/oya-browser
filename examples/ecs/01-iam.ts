// Run Oya Cloud browsers as Fargate tasks in your own AWS account, signed in with an IAM access key.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await oya.config.set({
  sandbox_runtime: 'ecs',
  ecs: {
    cluster: process.env.ECS_CLUSTER!,
    taskDefinition: process.env.ECS_TASK_DEFINITION!, // its container, 'browser', runs the Oya browser image
    subnets: process.env.ECS_SUBNETS!.split(','),
    assignPublicIp: true, // a public subnet without a NAT gateway needs this to pull the image
    region: process.env.AWS_REGION!,
    auth: {
      type: 'iam',
      accessKeyId: process.env.AWS_ACCESS_KEY_ID!,
      secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY!, // stored sealed, read back masked
    },
  },
});

await using browser = await oya.browser.start({ provider: 'oya-cloud' }); // a Fargate task in your account
await browser.goto('https://example.com');
console.log('Running on ECS at', await browser.url());
