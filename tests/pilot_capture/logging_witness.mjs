// Real native logger and real helper. The scoped preload observer is test-only.
import * as fs from "node:fs";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { logger } from "@oh-my-pi/pi-utils";
import { quiesceOwnedLogging } from "./startup_logging.ts";
export default async function loggingWitness(pi) {
 const s = globalThis[Symbol.for("cf-native-logging-probe")], kind = process.env.CF_STARTUP_WITNESS_KIND;
 const snapshot = () => ({ timers:s.timers.map(x=>({...x})), immediates:s.immediates.map(x=>({...x})) });
 const directory = process.env.PI_CODING_AGENT_DIR + "/../owned-logs";
 fs.mkdirSync(directory, { mode:0o700 });
 logger.flush(); s.scope="transport";
 logger.setTransports({ console:false, file:directory }); s.scope="marker";
 logger.info("CF_OWNED_LOG_BEFORE_RETIRE"); s.scope="none"; s.before=snapshot();
 if (kind==="logging-owned") await quiesceOwnedLogging();
 else if (kind==="logging-stop-only") logger.setTransports({ console:false, file:false });
 s.factory=snapshot();
 const dispose = logger.registerLogSink(event => {
  if (event.message!=="Model connection prewarm failed") return;
  s.tail.events++;
  s.tail.expected=event.level==="debug" && event.context?.origin==="https://api.anthropic.com" && event.context?.error==="CF_QUALIFICATION_STARTUP_REFUSED";
  queueMicrotask(()=>{s.tail.unwound++;dispose();s.tail.disposed++;});
 });
 if (kind==="logging-enabled") await new Promise(resolve=>setTimeout(resolve,1200));
 const serialized=JSON.stringify([getBundledModel("anthropic","claude-sonnet-5-5")]);
 if (Buffer.byteLength(serialized)>65536) throw new Error("logging fixture model cap");
 pi.registerProvider("anthropic", { fetchDynamicModels:async()=>JSON.parse(serialized) });
 pi.on("session_start",async(_event,ctx)=>{
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
  logger.flush();s.scope="marker";logger.info("CF_OWNED_LOG_AFTER_RETIRE");s.scope="none";
  s.session=snapshot();
 });
}
