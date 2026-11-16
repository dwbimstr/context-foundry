// Actual original native deadline; only the missing/unfinished controls inject faults.
import * as fs from "node:fs";
import * as https from "node:https";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { quiesceOwnedLogging } from "./startup_logging.ts";
export default async function warmupWitness(pi) {
 await quiesceOwnedLogging();
 const kind = process.env.CF_STARTUP_WITNESS_KIND;
 const serialized = JSON.stringify([getBundledModel("anthropic", "claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized) > 65536) throw new Error("warmup fixture model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels: async () => JSON.parse(serialized) });
 pi.on("session_start", async (_event, ctx) => {
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  const guard = globalThis[Symbol.for("context-foundry.qualification.warmup")];
  let witness;
  try { witness = { outcome: "sealed", summary: await guard.seal() }; }
  catch { witness = { outcome: "refused" }; }
  fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT, JSON.stringify(witness), { flag: "wx", mode: 0o600 });
  if (kind === "warmup-repeat-seal" || kind === "warmup-restart") {
   if (witness.outcome !== "sealed") throw new Error("warmup restart did not seal");
   setTimeout(() => {
    if (kind === "warmup-repeat-seal") void guard.seal();
    else https.request("https://api.anthropic.com/", { method: "HEAD" }).end();
   }, 500);
  }
 });
}
