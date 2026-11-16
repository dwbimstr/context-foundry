// Observe the original native startup task, preserving receivers and promises.
import * as fs from "node:fs";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { lookup } from "@oh-my-pi/pi-coding-agent/config/registry";
import { MarketplaceManager } from "@oh-my-pi/pi-coding-agent/extensibility/plugins/marketplace";

export default async function marketplaceWitness(pi) {
 const setting = lookup("marketplace.autoUpdate"), settings = Settings.instance;
 if (!setting || settings.getAgentDir() !== process.env.PI_CODING_AGENT_DIR) throw new Error("marketplace settings owner");
 const result = { factory_setting: setting.get(settings), session_setting: null, calls: [], overflow: false, methods_restored: false };
 const originals = new Map(), promises = [];
 for (const name of ["refreshStaleMarketplaces", "checkForUpdates"]) {
  const descriptor = Object.getOwnPropertyDescriptor(MarketplaceManager.prototype, name);
  if (!descriptor || typeof descriptor.value !== "function" || !descriptor.writable) throw new Error("marketplace method unavailable");
  const original = descriptor.value;
  originals.set(name, descriptor);
  Object.defineProperty(MarketplaceManager.prototype, name, { ...descriptor, value: function (...args) {
   if (result.calls.length >= 2) { result.overflow = true; throw new Error("marketplace call cap"); }
   const row = { name, native_receiver: this instanceof MarketplaceManager, arguments: args.length, completed: false, rejected: false };
   result.calls.push(row);
   const promise = Reflect.apply(original, this, args);
   if (!promise || typeof promise.then !== "function") throw new Error("marketplace native promise");
   promises.push(promise);
   promise.then(() => { row.completed = true; }, () => { row.rejected = true; });
   return promise;
  } });
 }
 process.once("exit", () => {
  for (const [name, descriptor] of originals) Object.defineProperty(MarketplaceManager.prototype, name, descriptor);
  result.methods_restored = true;
  result.original_promises_seen = promises.length;
  fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT, JSON.stringify(result), { flag: "wx", mode: 0o600 });
 });
 if (process.env.CF_STARTUP_WITNESS_KIND === "marketplace-late") {
  setting.set(settings, "off");
  await settings.flush();
 }
 const serialized = JSON.stringify([getBundledModel("anthropic", "claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized) > 65536) throw new Error("marketplace model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels: async () => JSON.parse(serialized) });
 pi.on("session_start", async (_event, ctx) => {
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  await new Promise(resolve => setTimeout(resolve, 250));
  await Promise.allSettled(promises);
  result.session_setting = setting.get(settings);
 });
}
