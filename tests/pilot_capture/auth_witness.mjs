// Actual SDK original-call controls. Faults replace metadata or completion only
// inside this fixture; they exercise no OAuth credential, refresh or provider.
import * as fs from "node:fs";
import { AuthStorage } from "@oh-my-pi/pi-ai";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { installOwnedPrefetchGuard } from "./startup_auth.ts";

export default async function authWitness(pi) {
 const kind = process.env.CF_STARTUP_WITNESS_KIND;
 if (["auth-oauth", "auth-unknown", "auth-unsettled"].includes(kind)) {
  const descriptor = Object.getOwnPropertyDescriptor(AuthStorage.prototype, "keys");
  let patched = false;
  Object.defineProperty(AuthStorage.prototype, "keys", { ...descriptor, get() {
   const keys = Reflect.apply(descriptor.get, this, []);
   if (!patched) {
    patched = true;
    const prototype = Object.getPrototypeOf(keys), prefetch = prototype.prefetchUsage, source = prototype.source;
    let inOriginal = false;
    prototype.source = function (...args) {
     return inOriginal && kind !== "auth-unsettled"
      ? { kind: kind === "auth-oauth" ? "oauth" : "unknown-fixture-kind", concrete: true }
      : Reflect.apply(source, this, args);
    };
    prototype.prefetchUsage = function (...args) {
     inOriginal = true;
     try {
      const original = Reflect.apply(prefetch, this, args);
      return kind === "auth-unsettled" ? new Promise(() => {}) : original;
     } finally { inOriginal = false; }
    };
   }
   return keys;
  } });
 }
 let guard = kind === "auth-late" ? undefined : await installOwnedPrefetchGuard();
 const serialized = JSON.stringify([getBundledModel("anthropic", "claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized) > 65536) throw new Error("auth fixture model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels: async () => JSON.parse(serialized) });
 pi.on("session_start", async (_event, ctx) => {
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  if (kind === "auth-late") guard = await installOwnedPrefetchGuard();
  let witness;
  try {
   if (kind === "auth-duplicate") ctx.modelRegistry.authStorage.keys.prefetchUsage("anthropic");
   const summary = guard.seal(kind === "auth-wrong-owner" ? {} : ctx.modelRegistry.authStorage.keys);
   witness = { outcome: "sealed", summary };
  } catch { witness = { outcome: "refused" }; }
  fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT, JSON.stringify(witness), { flag: "wx", mode: 0o600 });
  if (kind === "auth-restart") {
   if (witness.outcome !== "sealed") throw new Error("restart fixture did not seal");
   setTimeout(() => ctx.modelRegistry.authStorage.keys.prefetchUsage("anthropic"), 500);
  }
 });
}
