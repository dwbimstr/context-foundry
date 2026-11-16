/** Source-owned warm-up observation only. No Ready or generation API exists. */
declare const getCaptureChannel: typeof import("./bridge_channel").getCaptureChannel;
const STARTUP_STATE = Symbol.for("context-foundry.startup.refusal.v2");
const STARTUP_HOOK = Symbol.for("omp.connection-prewarm.refusal.v1");
const startupExact = (value: unknown, keys: string[]): value is Record<string, unknown> =>
 typeof value === "object" && value !== null && !Array.isArray(value) &&
 Object.keys(value).length === keys.length && keys.every(k => Object.hasOwn(value, k));
const startupFail = (): never => process.exit(75);
function createStartupObserver() {
 if (process.env.CF_PILOT_API !== "cf-pilot-startup-v2") startupFail();
 const channel = getCaptureChannel();
 let next = 0, bound = false, settled = false;
 const pending = new Map<number, { resolve: () => void; reject: () => void }>();
 const rejectAll = () => { for (const p of pending.values()) p.reject(); pending.clear(); };
 channel.attach((message, payload) => {
  if (payload.length || !startupExact(message, message?.type === "startup_refused" ?
   ["type", "attempt", "journal_seq", "journal_sha256"] : ["type", "reason"])) startupFail();
  if (message.type === "close") {
   if (message.reason !== "halted") startupFail();
   channel.close(); rejectAll(); return;
  }
  if (message.type !== "startup_refused" || !Number.isSafeInteger(message.attempt) ||
   !Number.isSafeInteger(message.journal_seq) || (message.journal_seq as number) <= 0 ||
   typeof message.journal_sha256 !== "string" || !/^[a-f0-9]{64}$/.test(message.journal_sha256)) startupFail();
  const waiting = pending.get(message.attempt as number);
  if (!waiting) startupFail();
  pending.delete(message.attempt as number); waiting.resolve();
 }, rejectAll);
 Object.defineProperty(globalThis, STARTUP_HOOK, {
  configurable: false, writable: false,
  value: async (metadata: unknown): Promise<void> => {
   if (settled || channel.quiet || !startupExact(metadata,["owner","method","origin"]) ||
    metadata.owner !== "model_connection_warmup" || metadata.method !== "HEAD" || metadata.origin !== "https://api.anthropic.com" || next === 20) {
    channel.send({type:"fault",code:"unclassified"}); startupFail();
   }
   const attempt = ++next;
   const refusal = new Promise<void>((resolve,reject) => pending.set(attempt,{resolve,reject:() => reject(new Error("startup closed"))}));
   if (!channel.send({type:"startup_attempt",attempt,owner:"model_connection_warmup",method:"HEAD",scheme:"https",hostname:"api.anthropic.com",port:443,route:"anthropic_origin"})) startupFail();
   await refusal;
  },
 });
 return Object.freeze({
  channel,
  bind(pi: { on(event: string, callback: (event: unknown, ctx: any) => Promise<void>): void }, sealRestricted: (manager: unknown, registry: unknown) => unknown) {
   if (bound || channel.quiet) startupFail(); bound = true;
   pi.on("session_start", async (_event, ctx) => {
    if (channel.quiet) return;
    if (settled || pending.size || next === 0) startupFail();
    let restriction: any;
    try {
     restriction = sealRestricted(ctx.sessionManager, ctx.modelRegistry);
    } catch { channel.send({type:"fault",code:"unclassified"}); startupFail(); }
    if (!startupExact(restriction,["tool_names","mcp_disabled","memory_disabled","credential_usage_joined"]) || restriction.mcp_disabled !== true || restriction.memory_disabled !== true || restriction.credential_usage_joined !== true || JSON.stringify(restriction.tool_names) !== JSON.stringify(["edit","read"])) startupFail();
    settled = true;
    if (!channel.send({type:"owner_settled",owner:"model_connection_warmup"}) || !channel.send({type:"restrictions_settled",...restriction})) startupFail();
   });
  },
 });
}
const startupGlobals = globalThis as unknown as Record<symbol, ReturnType<typeof createStartupObserver> | undefined>;
let startupOwner = startupGlobals[STARTUP_STATE];
if (startupOwner === undefined) {
 startupOwner = createStartupObserver();
 Object.defineProperty(globalThis,STARTUP_STATE,{value:startupOwner,configurable:false,writable:false});
} else if (startupOwner.channel !== getCaptureChannel()) startupFail();
export default async function observeStartup(pi: { on(event: string, callback: (event: unknown, ctx: any) => Promise<void>): void }): Promise<void> {
 // Only the late trusted-extension loader resolves host SDK imports. The early
 // preload owns the channel and receives this exact source binding afterward.
 const { sealRestrictedStartup } = await import("@oh-my-pi/pi-coding-agent");
 if (typeof sealRestrictedStartup !== "function") startupFail();
 startupOwner!.bind(pi, sealRestrictedStartup);
}
