// Actual pinned providers; only synthetic markers reach the owned loopback sink.
import * as fs from "node:fs";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { Settings } from "@oh-my-pi/pi-coding-agent/config/settings";
import { lookup } from "@oh-my-pi/pi-coding-agent/config/registry";
function providers() {
 const api = Object.getOwnPropertyDescriptor(globalThis, Symbol.for("opentelemetry.js.api.1"))?.value;
 const selector = Object.getOwnPropertyDescriptor(globalThis, Symbol.for("io.opentelemetry.js.api.logs"))?.value;
 if (api && (typeof api !== "object" || api.version !== "1.9.1")) throw new Error("OTLP fixture API identity");
 if (selector && typeof selector !== "function") throw new Error("OTLP fixture logs ABI");
 return { trace: api?.trace, metric: api?.metrics, log: selector?.(1) };
}
export default async function telemetryWitness(pi) {
 const initial = providers();
 if (initial.trace) {
  const span = initial.trace.getTracer("cf-owned-native-qualification").startSpan("cf-owned-native-registration");
  if (!span.isRecording()) throw new Error("OTLP fixture span not recording");
  span.end();
 }
 if (initial.metric) {
  initial.metric.getMeter("cf-owned-native-qualification").createCounter("cf_owned_native_metric").add(1);
  await initial.metric.forceFlush();
 }
 if (initial.log) {
  initial.log.getLogger("cf-owned-native-qualification").emit({ body: "cf-owned-native-log", severityNumber: 9, severityText: "INFO" });
  await initial.log.forceFlush();
 }
 const serialized = JSON.stringify([getBundledModel("anthropic", "claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized) > 65536) throw new Error("OTLP fixture model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels: async () => JSON.parse(serialized) });
 pi.on("session_start", async (_event, ctx) => {
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  const final = providers();
  const witness = { export_setting: lookup("telemetry.otlpExportEnabled")?.get(Settings.instance),
   trace_provider_present: !!final.trace, metric_provider_present: !!final.metric, log_provider_present: !!final.log,
   globals_stable: Object.keys(initial).every(key => initial[key] === final[key]) };
  fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT, JSON.stringify(witness), { flag: "wx", mode: 0o600 });
 });
}
