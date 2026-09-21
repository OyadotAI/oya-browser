import { Oya } from "@oya-ai/browser";

const oya = new Oya();
const persona = await oya.personas.create({ name: "portal-ops" });

await oya.personas.setCredentials(persona.id, { domain: "portal.example.com", username: "alice", password: process.env.PORTAL_PASSWORD! });
await oya.personas.setMfa(persona.id, { domain: "portal.example.com", type: "gmail", refreshToken: process.env.MAIL_REFRESH_TOKEN!, clientId: process.env.MAIL_CLIENT_ID! });
