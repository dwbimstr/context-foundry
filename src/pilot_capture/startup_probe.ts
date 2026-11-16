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
