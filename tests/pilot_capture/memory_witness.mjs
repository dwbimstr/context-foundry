// Actual SDK off-backend startup. Eligibility/completion faults are synthetic;
// active backends and auto-learn are restored before SDK construction resumes.
import * as fs from "node:fs";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { offBackend } from "@oh-my-pi/pi-coding-agent/memory-backend";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { lookup } from "@oh-my-pi/pi-coding-agent/config/registry";
import { installOwnedMemoryGuard } from "./startup_memory.ts";

export default async function memoryWitness(pi) {
 const kind = process.env.CF_STARTUP_WITNESS_KIND, settings = Settings.instance;
 if (kind === "memory-unsettled") {
  const original = offBackend.start;
  offBackend.start = function (...args) { Reflect.apply(original, this, args); return new Promise(() => {}); };
 }
 let guard, earlyRefusal = false;
 const faultSetting = kind === "memory-active-backend" ? lookup("memory.backend")
  : kind === "memory-auto-learn" ? lookup("autolearn.enabled") : undefined;
 if (faultSetting) faultSetting.override(settings, kind === "memory-active-backend" ? "local" : true);
 try { if (kind !== "memory-late") guard = await installOwnedMemoryGuard(); }
 catch { earlyRefusal = true; }
 finally { if (faultSetting) faultSetting.override(settings, kind === "memory-active-backend" ? "off" : false); }
 const serialized = JSON.stringify([getBundledModel("anthropic", "claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized) > 65536) throw new Error("memory fixture model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels: async () => JSON.parse(serialized) });
 pi.on("session_start", async (_event, ctx) => {
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  let witness;
  try {
   if (kind === "memory-late") guard = await installOwnedMemoryGuard();
   if (earlyRefusal) witness = { outcome: "before_sdk_refused" };
   else {
    if (kind === "memory-duplicate") offBackend.start({ settings, modelRegistry: ctx.modelRegistry });
    const summary = await guard.seal(kind === "memory-wrong-settings" ? {} : settings,
     kind === "memory-wrong-registry" ? {} : ctx.modelRegistry);
    witness = { outcome: "sealed", summary };
   }
  } catch { witness = { outcome: "refused" }; }
  fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT, JSON.stringify(witness), { flag: "wx", mode: 0o600 });
  if (kind === "memory-restart") {
   if (witness.outcome !== "sealed") throw new Error("memory restart did not seal");
   setTimeout(() => offBackend.start({ settings, modelRegistry: ctx.modelRegistry }), 500);
  }
 });
}
