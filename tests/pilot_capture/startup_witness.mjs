// Native SDK control only: no prompt, real credential or provider request.
import * as fs from "node:fs";
import * as https from "node:https";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";

export default function startupWitness(pi) {
 const serialized = JSON.stringify([getBundledModel("anthropic", "claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized) > 65536) throw new Error("startup fixture model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels: async () => JSON.parse(serialized) });
 pi.on("session_start", async (_event, ctx) => {
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  const url = new URL(process.env.CF_STARTUP_WITNESS_URL);
  let outcome;
  if (process.env.CF_STARTUP_WITNESS_KIND === "raw") {
   try {
    const socket = await Bun.connect({ hostname: url.hostname, port: Number(url.port),
     socket: { data() {}, error() {} } });
    socket.end();
    outcome = "raw_open";
   } catch { outcome = "native_error"; }
  } else {
   outcome = await new Promise(resolve => {
    let request, timer;
    const done = value => { if (timer) clearTimeout(timer); resolve(value); };
    try {
     request = https.request(url, { method: "HEAD", headers: {}, agent: new https.Agent({ keepAlive: false }) });
     request.once("error", error => done(error?.message === "CF_QUALIFICATION_STARTUP_REFUSED" ? "owned_refusal" : "native_error"));
     request.once("response", response => { response.destroy(); done("response"); });
     timer = setTimeout(() => { request.destroy(); done("timeout"); }, 1000);
     request.end();
    } catch (error) {
     request?.destroy();
     done(error?.message === "CF_QUALIFICATION_STARTUP_REFUSED" ? "owned_refusal" : "throw");
    }
   });
  }
  fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT,
   JSON.stringify({ outcome }), { flag: "wx", mode: 0o600 });
 });
}
