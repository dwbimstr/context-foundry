/** Source-owned startup transport observation. No Ready or generation API exists. */
declare const getCaptureChannel: typeof import("./bridge_channel").getCaptureChannel;
const STARTUP_STATE = Symbol.for("context-foundry.startup.refusal.v2");
const STARTUP_HOOK = Symbol.for("omp.connection-prewarm.refusal.v1");
const CATALOG_HOOK = Symbol.for("context-foundry.startup.catalog-refusal.v1");
type StartupAttemptOwner = "model_connection_warmup" | "model_catalog_discovery";
const startupExact = (value: unknown, keys: string[]): value is Record<string, unknown> =>
 typeof value === "object" && value !== null && !Array.isArray(value) &&
 Object.keys(value).length === keys.length && keys.every(k => Object.hasOwn(value, k));
const startupFail = (): never => process.exit(75);
function createStartupObserver() {
 if (process.env.CF_PILOT_API !== "cf-pilot-startup-v2") startupFail();
 const channel = getCaptureChannel();
 let next = 0, refused = 0, lastJournalSeq = 0, bound = false, settled = false, pluginDone = false;
 let extensionDone = false, catalogDone = false, warmupSeen = false;
 const seenJournalDigests = new Set<string>();
 let consumeExtension: ((proof: unknown) => unknown) | undefined;
 let consumeRpc: ((proof: unknown) => unknown) | undefined;
 const pending = new Map<number, { owner: StartupAttemptOwner; resolve: () => void; reject: () => void }>();
 const pendingFor = (owner: StartupAttemptOwner) => [...pending.values()].some(p => p.owner === owner);
 const rejectAll = () => { for (const p of pending.values()) p.reject(); pending.clear(); };
 const fail = (): never => { channel.close(); rejectAll(); return startupFail(); };
 channel.attach((message, payload) => {
  if (payload.length || !startupExact(message, message?.type === "startup_refused" ?
   ["type", "attempt", "journal_seq", "journal_sha256"] : ["type", "reason"])) fail();
  if (message.type === "close") {
   if (message.reason !== "halted") fail();
   channel.close(); rejectAll(); return;
  }
  if (message.type !== "startup_refused" || !Number.isSafeInteger(message.attempt) ||
   message.attempt !== refused + 1 || !Number.isSafeInteger(message.journal_seq) ||
   (message.journal_seq as number) <= lastJournalSeq || typeof message.journal_sha256 !== "string" ||
   !/^[a-f0-9]{64}$/.test(message.journal_sha256) || seenJournalDigests.has(message.journal_sha256)) fail();
  const waiting = pending.get(message.attempt as number);
  if (!waiting) fail();
  refused = message.attempt as number; lastJournalSeq = message.journal_seq as number; seenJournalDigests.add(message.journal_sha256);
  pending.delete(message.attempt as number); waiting.resolve();
 }, rejectAll);
 Object.defineProperty(globalThis, Symbol.for("context-foundry.startup.extension-settled.v1"), {
  configurable:false,writable:false,
  value:(proof:unknown):void => {
   if (!bound || !settled || !pluginDone || extensionDone || channel.quiet || !consumeExtension) fail();
   let extension:any;
   try { extension=consumeExtension(proof); }
   catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
   if (!startupExact(extension,["handlers_joined","callbacks_joined","starts_closed","managed_timers_excluded","handlers"]) || extension.handlers_joined !== true || extension.callbacks_joined !== true || extension.starts_closed !== true || extension.managed_timers_excluded !== true || !Number.isInteger(extension.handlers) || extension.handlers < 1 || extension.handlers > 64) fail();
   if (!channel.send({type:"extension_retired",...extension})) fail();
   extensionDone = true;
  },
 });
 Object.defineProperty(globalThis, Symbol.for("context-foundry.startup.rpc-retired.v1"), {
  configurable:false,writable:false,
  value:(proof:unknown):void => {
   if (!extensionDone || channel.quiet || !consumeRpc) fail();
   let rpc: unknown;
   try { rpc = consumeRpc(proof); }
   catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
   const facts = ["initial_catalog_joined","metadata_subscription_excluded","output_drained","dispatch_closed"];
   if (!startupExact(rpc,facts) || !facts.every(key => rpc[key] === true)) fail();
   if (!channel.send({type:"rpc_retired",...rpc})) fail();
  },
 });
 const reportRefusal = async (owner: StartupAttemptOwner, metadata: unknown): Promise<void> => {
  const warmup = owner === "model_connection_warmup";
  const method = warmup ? "HEAD" : "GET";
  const hostname = warmup ? "api.anthropic.com" : "catalog.stencil.so";
  const route = warmup ? "anthropic_origin" : "stencil_catalog";
  if ((warmup ? settled : catalogDone) || channel.quiet || next === 20 ||
   !startupExact(metadata, ["owner", "method", "origin"]) || metadata.owner !== owner ||
   metadata.method !== method || metadata.origin !== `https://${hostname}`) {
   channel.send({type:"fault",code:"unclassified"}); fail();
  }
  if (warmup) warmupSeen = true;
  const attempt = ++next;
  const refusal = new Promise<void>((resolve,reject) => pending.set(attempt,{owner,resolve,reject:() => reject(new Error("startup closed"))}));
  if (!channel.send({type:"startup_attempt",attempt,owner,method,scheme:"https",hostname,port:443,route})) fail();
  await refusal;
 };
 Object.defineProperty(globalThis, STARTUP_HOOK, {
  configurable:false,writable:false,
  value:(metadata:unknown) => reportRefusal("model_connection_warmup",metadata),
 });
 Object.defineProperty(globalThis, CATALOG_HOOK, {
  configurable:false,writable:false,
  value:(metadata:unknown) => reportRefusal("model_catalog_discovery",metadata),
 });
 return Object.freeze({
  channel,
  bind(pi: { on(event: string, callback: (event: unknown, ctx: any) => Promise<void>): void }, sealRestricted: (manager: unknown, registry: unknown) => unknown, retireCatalog: (manager: unknown, registry: unknown) => Promise<unknown>, retireSettings: (manager: unknown, registry: unknown) => Promise<unknown>, retireAuth: (manager: unknown, registry: unknown) => Promise<unknown>, retirePresence: (manager: unknown, registry: unknown) => Promise<unknown>, retireTelemetry: (manager: unknown, registry: unknown) => Promise<unknown>, retireMarketplace: (manager: unknown, registry: unknown) => Promise<unknown>, retireLogging: (manager: unknown, registry: unknown) => Promise<unknown>, retirePluginRoots: (manager: unknown, registry: unknown) => Promise<unknown>, consume: (proof: unknown) => unknown, consumeRpcReceipt: (proof: unknown) => unknown) {
   if (bound || channel.quiet) fail(); bound = true; consumeExtension = consume; consumeRpc = consumeRpcReceipt;
   pi.on("session_start", async (_event, ctx) => {
    if (channel.quiet) return;
    if (settled || pendingFor("model_connection_warmup") || !warmupSeen) fail();
    let restriction: any;
    try {
     restriction = sealRestricted(ctx.sessionManager, ctx.modelRegistry);
    } catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(restriction,["tool_names","mcp_disabled","memory_disabled","credential_usage_joined"]) || restriction.mcp_disabled !== true || restriction.memory_disabled !== true || restriction.credential_usage_joined !== true || JSON.stringify(restriction.tool_names) !== JSON.stringify(["edit","read"])) fail();
    settled = true;
    if (!channel.send({type:"owner_settled",owner:"model_connection_warmup"}) || !channel.send({type:"restrictions_settled",...restriction})) fail();
    let catalog: any;
    try { catalog = await retireCatalog(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(catalog,["discovery_retired","originals_joined","callbacks_joined","timers_retired","operations","rejections"]) || catalog.discovery_retired !== true || catalog.originals_joined !== true || catalog.callbacks_joined !== true || catalog.timers_retired !== true || !Number.isInteger(catalog.operations) || catalog.operations < 1 || catalog.operations > 4096 || !Number.isInteger(catalog.rejections) || catalog.rejections < 0 || catalog.rejections > catalog.operations) fail();
    if (pendingFor("model_catalog_discovery")) fail();
    catalogDone = true;
    if (!channel.quiet && !channel.send({type:"catalog_retired",...catalog})) fail();
    let settings: any;
    try { settings = await retireSettings(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(settings,["auth_listener_retired","settings_retired","originals_joined","callbacks_joined","timers_retired","operations","rejections"]) || settings.auth_listener_retired !== true || settings.settings_retired !== true || settings.originals_joined !== true || settings.callbacks_joined !== true || settings.timers_retired !== true || !Number.isInteger(settings.operations) || settings.operations < 1 || settings.operations > 4096 || !Number.isInteger(settings.rejections) || settings.rejections < 0 || settings.rejections > settings.operations) fail();
    if (!channel.quiet && !channel.send({type:"settings_retired",...settings})) fail();
    let auth: any;
    try { auth = await retireAuth(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(auth,["empty_auth","auth_retired","originals_joined","callbacks_joined","store_closed","operations","rejections"]) || auth.empty_auth !== true || auth.auth_retired !== true || auth.originals_joined !== true || auth.callbacks_joined !== true || auth.store_closed !== true || !Number.isInteger(auth.operations) || auth.operations < 1 || auth.operations > 4096 || !Number.isInteger(auth.rejections) || auth.rejections < 0 || auth.rejections > auth.operations) fail();
    if (!channel.quiet && !channel.send({type:"auth_retired",...auth})) fail();
    let presence: any;
    try { presence = await retirePresence(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(presence,["presence_retired","registration_joined","callbacks_joined","cleanup_joined","presence_removed"]) || presence.presence_retired !== true || presence.registration_joined !== true || presence.callbacks_joined !== true || presence.cleanup_joined !== true || presence.presence_removed !== true) fail();
    if (!channel.quiet && !channel.send({type:"presence_retired",...presence})) fail();
    let telemetry: any;
    try { telemetry = await retireTelemetry(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(telemetry,["telemetry_disabled","original_joined","callbacks_joined"]) || telemetry.telemetry_disabled !== true || telemetry.original_joined !== true || telemetry.callbacks_joined !== true) fail();
    if (!channel.quiet && !channel.send({type:"telemetry_retired",...telemetry})) fail();
    let marketplace: any;
    try { marketplace = await retireMarketplace(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(marketplace,["marketplace_disabled","schedule_completed"]) || marketplace.marketplace_disabled !== true || marketplace.schedule_completed !== true) fail();
    if (!channel.quiet && !channel.send({type:"marketplace_retired",...marketplace})) fail();
    let logging: any;
    try { logging = await retireLogging(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(logging,["logging_closed","transport_closed","retention_joined"]) || logging.logging_closed !== true || logging.transport_closed !== true || logging.retention_joined !== true) fail();
    if (!channel.quiet && !channel.send({type:"logging_retired",...logging})) fail();
    let plugin: any;
    try { plugin = await retirePluginRoots(ctx.sessionManager, ctx.modelRegistry); }
    catch { channel.send({type:"fault",code:"unclassified"}); fail(); }
    if (!startupExact(plugin,["plugin_roots_closed","originals_joined","callbacks_joined","operations"]) || plugin.plugin_roots_closed !== true || plugin.originals_joined !== true || plugin.callbacks_joined !== true || !Number.isInteger(plugin.operations) || plugin.operations < 1 || plugin.operations > 4096) fail();
    if (channel.quiet || !channel.send({type:"plugin_roots_retired",...plugin})) fail();
    pluginDone = true;
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
 const { sealRestrictedStartup, retireRestrictedStartupCatalog, retireRestrictedStartupSettings, retireRestrictedStartupAuth, retireRestrictedStartupPresence, retireRestrictedStartupTelemetry, retireRestrictedStartupMarketplace, retireRestrictedStartupLogging, retireRestrictedStartupPluginRoots, consumeStartupExtensionReceipt } = await import("@oh-my-pi/pi-coding-agent/sdk");
 const { consumeStartupRpcReceipt } = await import("@oh-my-pi/pi-coding-agent/modes/rpc/rpc-mode");
 if (typeof sealRestrictedStartup !== "function" || typeof retireRestrictedStartupCatalog !== "function" || typeof retireRestrictedStartupSettings !== "function" || typeof retireRestrictedStartupAuth !== "function" || typeof retireRestrictedStartupPresence !== "function" || typeof retireRestrictedStartupTelemetry !== "function" || typeof retireRestrictedStartupMarketplace !== "function" || typeof retireRestrictedStartupLogging !== "function" || typeof retireRestrictedStartupPluginRoots !== "function" || typeof consumeStartupExtensionReceipt !== "function") startupFail();
 if (typeof consumeStartupRpcReceipt !== "function") startupFail();
 startupOwner!.bind(pi, sealRestrictedStartup, retireRestrictedStartupCatalog, retireRestrictedStartupSettings, retireRestrictedStartupAuth, retireRestrictedStartupPresence, retireRestrictedStartupTelemetry, retireRestrictedStartupMarketplace, retireRestrictedStartupLogging, retireRestrictedStartupPluginRoots, consumeStartupExtensionReceipt, consumeStartupRpcReceipt);
}
