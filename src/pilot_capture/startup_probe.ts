/**
 * Metadata-only diagnostic for the artifact-pinned, no-provider qualifier.
 * The fixed qualifier gates capture Ready on one identified native deadline.
 * This grants no full startup admission; its trace is not the capture journal.
 * The known Node HTTP Agent route is refused before its TCP connection.
 * Raw sockets and other transports remain outside this diagnostic's coverage.
 */
import * as fs from "node:fs";
import http from "node:http";
import https from "node:https";

const fail = (code = 75): never => process.exit(code);
const workspace = process.env.CF_PILOT_PROBE_WORKSPACE;
const trace = process.env.CF_PILOT_PROBE_TRACE;
if (!workspace?.startsWith("/") || !trace?.startsWith("/")) fail();
let fd: number;
try {
	process.chdir(workspace);
	if (process.cwd() !== workspace) fail();
	fd = fs.openSync(trace, "ax", 0o600);
} catch { fail(); }

let sequence = 0;
let bytes = 0;
let attempts = 0;
function record(event: Record<string, unknown>): void {
	const line = Buffer.from(JSON.stringify({ sequence: sequence++, ...event }) + "\n");
	bytes += line.length;
	if (sequence > 26 || bytes > 8192) fail(74);
	try {
		let offset = 0;
		while (offset < line.length) {
			const written = fs.writeSync(fd, line, offset, line.length - offset);
			if (written <= 0) fail();
			offset += written;
		}
		fs.fsyncSync(fd);
	} catch { fail(); }
}

function observe(transport: string, input: unknown, method: unknown): void {
	if (++attempts > 20) fail(74);
	let url: URL;
	try {
		const target = typeof input === "string" ? input : input instanceof URL ? input.href : input instanceof Request ? input.url : undefined;
		if (target === undefined) fail();
		url = new URL(target);
	} catch { fail(); }
	if ((url.protocol !== "https:" && url.protocol !== "http:") || !url.hostname || url.hostname.length > 253 ||
		typeof method !== "string" || !/^[A-Z]{1,16}$/.test(method)) fail();
	const route = url.hostname === "api.anthropic.com" && url.pathname === "/" && method === "HEAD" ? "anthropic_connection_prewarm" :
		url.hostname === "catalog.stencil.so" && url.pathname === "/models.json.zstd" ? "shared_catalog" : "other";
	record({ kind: "attempt", transport, method, scheme: url.protocol, host: url.hostname, port: url.port, route });
}

// Observe only instances returned by the original 10-second native timeout.
// A signal is owned only when the SDK removes its exact callback while our
// classified request error is delivered. No native timer is replaced or cancelled.
const nativeTimeout = AbortSignal.timeout;
const nativeAdd = AbortSignal.prototype.addEventListener;
const nativeRemove = AbortSignal.prototype.removeEventListener;
type NativeSignal = { listener?: EventListenerOrEventListenerObject | null; removed: boolean };
const signals = new WeakMap<AbortSignal, NativeSignal>();
let candidates = 0;
let knownRequests = 0;
let deliveringKnownRefusal = false;
let bound: AbortSignal | undefined;
let elapsed = false;
let observerRemoved = false;
let sealing = false;
let loggingDone = false;
let sealed = false;
let failed = false;
let resolveDeadline!: () => void;
const deadline = new Promise<void>(resolve => { resolveDeadline = resolve; });
function refuseWarmup(): never {
 failed = true;
 if (sealed) return fail();
 throw new Error("owned warmup refused");
}
function warmupState(state: string): void {
 record({ kind: "warmup", state, native_timeout_ms: 10000 });
}
Object.defineProperty(AbortSignal, "timeout", {
 configurable: false, writable: false,
 value: function (this: typeof AbortSignal, ...args: Parameters<typeof AbortSignal.timeout>) {
  const signal = Reflect.apply(nativeTimeout, this, args);
  if (args.length !== 1 || args[0] !== 10000) return signal;
  if (++candidates > 20) fail(74);
  const state: NativeSignal = { removed: false };
  signals.set(signal, state);
  Object.defineProperty(signal, "addEventListener", {
   configurable: false, writable: false,
   value: function (this: AbortSignal, ...args: Parameters<AbortSignal["addEventListener"]>) {
    if (this !== signal) return refuseWarmup();
    if (args[0] === "abort") {
     if (state.listener !== undefined || state.removed || typeof args[1] !== "function") return refuseWarmup();
     state.listener = args[1];
    }
    return Reflect.apply(nativeAdd, this, args);
   },
  });
  Object.defineProperty(signal, "removeEventListener", {
   configurable: false, writable: false,
   value: function (this: AbortSignal, ...args: Parameters<AbortSignal["removeEventListener"]>) {
    if (this !== signal) return refuseWarmup();
    const result = Reflect.apply(nativeRemove, this, args);
    if (args[0] === "abort" && deliveringKnownRefusal) {
     if (failed || sealing || sealed || bound || state.removed || !state.listener || args[1] !== state.listener || this.aborted) return refuseWarmup();
     bound = this;
     state.removed = true;
     state.listener = undefined;
     warmupState("listener_released");
     const completed = () => {
      Reflect.apply(nativeRemove, signal, ["abort", completed]);
      observerRemoved = true;
      if (failed || elapsed || !signal.aborted || signal.reason?.name !== "TimeoutError") return refuseWarmup();
      elapsed = true;
      warmupState("deadline_elapsed");
      resolveDeadline();
     };
     Reflect.apply(nativeAdd, this, ["abort", completed, { once: true }]);
    }
    return result;
   },
  });
  return signal;
 },
});
Object.defineProperty(globalThis, Symbol.for("context-foundry.qualification.warmup"), {
 configurable: false, writable: false,
 value: Object.freeze({
  loggingQuiesced() {
   if (failed || sealed || loggingDone) return refuseWarmup();
   loggingDone = true;
   record({ kind: "logging", state: "quiesced" });
  },
  async seal() {
   if (failed || sealing || sealed || !loggingDone || knownRequests !== 1 || !bound || !signals.get(bound)?.removed) return refuseWarmup();
   sealing = true;
   let timer: ReturnType<typeof setTimeout> | undefined;
   try {
    await Promise.race([deadline, new Promise<never>((_resolve, reject) => {
     timer = setTimeout(() => reject(new Error("owned warmup deadline unfinished")), 12000);
    })]);
    if (failed || !elapsed || !observerRemoved || !bound.aborted || bound.reason?.name !== "TimeoutError") return refuseWarmup();
   } catch { return refuseWarmup(); }
   finally { if (timer !== undefined) clearTimeout(timer); }
   sealed = true;
   warmupState("sealed");
   return Object.freeze({ known_requests: knownRequests, listener_released: true,
    native_timeout_ms: 10000, deadline_elapsed: elapsed, observer_removed: observerRemoved });
  },
 }),
});

type FetchWithPreconnect = typeof globalThis.fetch & { preconnect?: (input: unknown) => unknown };
const originalFetch = globalThis.fetch as FetchWithPreconnect;
const refuseFetch = (async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
	observe("global_fetch", input, init?.method ?? (input instanceof Request ? input.method : "GET"));
	throw new Error("CF_QUALIFICATION_STARTUP_REFUSED");
}) as FetchWithPreconnect;
if (typeof originalFetch.preconnect === "function") {
	refuseFetch.preconnect = (input: unknown): never => {
		observe("fetch_preconnect", input, "CONNECT");
		throw new Error("CF_QUALIFICATION_STARTUP_REFUSED");
	};
}
globalThis.fetch = refuseFetch;

// The Agent queue runs before the known SDK route opens a TCP connection.
// Only the Host destination is read; no other header or body is inspected.
interface RequestMetadata {
	protocol?: string;
	agent?: { protocol?: string };
	host?: string;
	path?: string;
	method?: string;
	getHeader(name: string): unknown;
	destroy(): unknown;
	emit(event: "error", error: Error): unknown;
}
for (const prototype of new Set([http.Agent.prototype, https.Agent.prototype])) {
	if (typeof prototype.addRequest !== "function") fail();
	Object.defineProperty(prototype, "addRequest", {
		configurable: false, writable: false,
		value(this: { protocol?: string }, request: RequestMetadata, options: { protocol?: string; hostname?: string; host?: string; port?: number }) {
			const scheme = this.protocol ?? request.protocol ?? request.agent?.protocol ?? options.protocol;
			const header = request.getHeader("host");
			const host = header ?? options.hostname ?? options.host;
			if ((scheme !== "http:" && scheme !== "https:") || typeof host !== "string") fail();
			const port = header === undefined && options.port !== undefined ? `:${options.port}` : "";
			observe("agent_add_request", `${scheme}//${host}${port}${request.path ?? "/"}`, request.method);
			if (typeof request.destroy !== "function" || typeof request.emit !== "function") fail();
            const known = scheme === "https:" && host === "api.anthropic.com" && port === "" && request.method === "HEAD" && request.path === "/";
            if (known && (sealed || failed || ++knownRequests !== 1)) return refuseWarmup();
			// Bun's socketless request does not surface destroy(error). After
			// construction, retire it and deliver the refusal to the SDK's error
			// handler so it releases its abort listener. Never queue network IO.
			queueMicrotask(() => {
				request.destroy();
                deliveringKnownRefusal = known;
                try { request.emit("error", new Error("CF_QUALIFICATION_STARTUP_REFUSED")); }
                finally { deliveringKnownRefusal = false; }
			});
		},
	});
}

record({ kind: "installed", runtime: Bun.version, workspace_restored: true });
process.on("exit", code => {
	record({ kind: "exit", code });
	fs.closeSync(fd);
});

// Original CLI auth owner only. No credential values or replacement sync handle.
type AuthPair = { source: Promise<unknown>; returned: Promise<unknown>; done: boolean; rejected: boolean };
const authAddDescriptor = Object.getOwnPropertyDescriptor(Set.prototype, "add")!;
const authCatchDescriptor = Object.getOwnPropertyDescriptor(Promise.prototype, "catch")!;
const authThenDescriptor = Object.getOwnPropertyDescriptor(Promise.prototype, "then")!;
const authAdd = authAddDescriptor.value, authCatch = authCatchDescriptor.value, authThen = authThenDescriptor.value;
const authHas = Set.prototype.has, authDelete = Set.prototype.delete;
const authSubscriptions: { set: Set<unknown>; callback: unknown }[] = [];
const authInitial: AuthPair[] = [], authPending: AuthPair[] = [];
let authBound = false, authRetiring = false, authRetired = false, authFailed = false;
function authRefuse(): never {
 authFailed = true;
 if (authRetired) fail(75);
 throw new Error("owned auth retirement refused");
}
function authHash(callback: unknown, cap: number): string | undefined {
 if (typeof callback !== "function") return;
 const source = Function.prototype.toString.call(callback);
 if (source.length > cap) return;
 return new Bun.CryptoHasher("sha256").update(source).digest("hex");
}
function authFrame(fragment: string): boolean { return (new Error().stack ?? "").includes(fragment); }
function authWatch(source: Promise<unknown>, returned: Promise<unknown>): AuthPair {
 if (!(source instanceof Promise) || !(returned instanceof Promise)) return authRefuse();
 const pair = { source, returned, done: false, rejected: false };
 Reflect.apply(authThen, source, [() => {}, () => { pair.rejected = true; }]);
 Reflect.apply(authThen, returned, [() => { pair.done = true; }, () => { pair.rejected = true; }]);
 return pair;
}
Object.defineProperty(Set.prototype, "add", { ...authAddDescriptor, value: function (this: Set<unknown>, ...args: unknown[]) {
 const target = authHash(args[0], 1024) === "ada5cdc854c45871bde0164645643391deae2672cb41dc2f6360ec797c33aef8" &&
  authFrame("at aee (/$bunfs/root/chunks/cli-3yasrrk4.js:115:21)");
 if (target && (authRetired || authFailed || authSubscriptions.length >= 4 ||
  authSubscriptions.some(x => x.set === this) || (authSubscriptions.length && authSubscriptions[0].callback !== args[0]))) return authRefuse();
 const returned = Reflect.apply(authAdd, this, args);
 if (target) {
  if (!(this instanceof Set) || returned !== this) return authRefuse();
  authSubscriptions.push({ set: this, callback: args[0] });
 }
 return returned;
} });
Object.defineProperty(Promise.prototype, "catch", { ...authCatchDescriptor, value: function (this: Promise<unknown>, ...args: unknown[]) {
 const target = typeof args[0] === "function" && Function.prototype.toString.call(args[0]).replace(/\s/g, "") === "()=>null" &&
  authFrame("at aee (/$bunfs/root/chunks/cli-3yasrrk4.js:87:22)");
 if (target && (authRetired || authFailed || authInitial.length !== 0)) return authRefuse();
 const returned = Reflect.apply(authCatch, this, args);
 if (target) authInitial.push(authWatch(this, returned));
 return returned;
} });
Object.defineProperty(Promise.prototype, "then", { ...authThenDescriptor, value: function (this: Promise<unknown>, ...args: unknown[]) {
 const target = authHash(args[0], 256) === "73e10fcea65a780ac7077e5857e780e8442086c285b36d9c75c345e4e634602c" &&
  authFrame("at <anonymous> (/$bunfs/root/chunks/cli-3yasrrk4.js:118:15)");
 if (target && (authRetired || authFailed || authPending.length >= 8)) return authRefuse();
 const returned = Reflect.apply(authThen, this, args);
 if (target) authPending.push(authWatch(this, returned));
 return returned;
} });
Object.defineProperty(globalThis, Symbol.for("context-foundry.qualification.auth"), { value: Object.freeze({
 bind() {
  if (authBound || authFailed || authInitial.length !== 1 || authSubscriptions.length !== 4 ||
   authSubscriptions.some(x => !Reflect.apply(authHas, x.set, [x.callback]))) return authRefuse();
  authBound = true;
  record({ kind: "auth", state: "registered", subscriptions: 4 });
 },
 async retire(closeOriginal: () => void) {
  if (!authBound || authRetiring || authRetired || authFailed) return authRefuse();
  authRetiring = true;
  for (const entry of authSubscriptions) {
   if (!Reflect.apply(authDelete, entry.set, [entry.callback]) || Reflect.apply(authHas, entry.set, [entry.callback])) return authRefuse();
  }
  // Removing a registration does not cancel its already queued microtask.
  await new Promise<void>(resolve => setImmediate(resolve));
  const count = authPending.length, pairs = [...authInitial, ...authPending];
  try { await Promise.all(pairs.flatMap(pair => [pair.source, pair.returned])); } catch { return authRefuse(); }
  if (authFailed || authPending.length !== count || pairs.some(pair => !pair.done || pair.rejected)) return authRefuse();
  closeOriginal();
  if (authFailed) return authRefuse();
  authRetired = true;
  record({ kind: "auth", state: "retired", subscriptions: 4, continuations: count, initial_resolver_joined: true, store_closed: true });
  authSubscriptions.length = 0; authInitial.length = 0; authPending.length = 0;
  return Object.freeze({ subscriptions: 4, continuations: count, initial_resolver_joined: true, store_closed: true });
 },
}) });
