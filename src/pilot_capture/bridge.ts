/**
 * Context Foundry bounded-pilot capture bridge (`cf-pilot-capture-v1`).
 *
 * Purpose: the thin native OMP extension the pinned host requires. It registers
 * the custom Anthropic API handler, selects the frozen model clone, thinking
 * level and tools, delegates request building and SSE parsing to the
 * host-bundled `streamAnthropic`, and relays every provider fetch and every tool
 * admission over a private framed channel to the Rust capture session.
 *
 * Boundary: credential-free (the only key here is the synthetic SDK key that
 * Rust verifies and replaces), network-denied (the host is sandboxed and the
 * delegate's fetch is always the relay below), no file writes, no budgets, no
 * retries. Rust is the only authority over fetches, response bytes and tool
 * calls: the bridge never refuses on counts, never invents an HTTP status and
 * never surfaces an upstream body. Doubt fails closed: fault, abort, exit.
 *
 * Contract (schema: src/pilot_capture/protocol.rs). CF_PILOT_SESSION and
 * CF_PILOT_HOST are echoed in every header; CF_PILOT_TX_FD="3" is the inherited
 * child->Rust FIFO (whole frames, blocking writeSync); CF_PILOT_RX_FD="4" is the
 * inherited Rust->child FIFO (read asynchronously); CF_PILOT_TX_INO and
 * CF_PILOT_RX_INO name the exact pipe inodes Rust created (an fd that is
 * merely some FIFO is refused); CF_PILOT_API must be
 * "cf-pilot-capture-v1". A frame is: u32 LE header length, compact JSON header
 * {v:1,session,host,seq,msg}, u32 LE payload length, payload (header 1..=65536
 * bytes, payload <= 1 MiB, seq from 1 per direction). Invalid configuration
 * throws at load, so no Ready is sent. Exit codes: 70 channel EOF before close,
 * 71 channel write failure, 72 protocol violation or internal bridge failure.
 */
import * as fs from "node:fs";
import { createHash } from "node:crypto";
import { streamAnthropic } from "@oh-my-pi/pi-ai/providers/register-builtins";

const API = "cf-pilot-capture-v1";
const PROVIDER = "anthropic";
const MODEL = "claude-sonnet-5-5";
const DELEGATE_API = "anthropic-messages";
const MAX_TOKENS = 4096;
const THINKING_LEVEL = "xhigh";
const EFFORT = "xhigh";
const TOOL_NAMES: readonly string[] = ["read", "edit"];
const SERVICE_TIER = "standard_only";
const SYNTHETIC_KEY = "sk-ant-api03-cf-pilot-bridge-synthetic-not-a-key";
const TX_FD = 3;
const RX_FD = 4;
const PROTOCOL_VERSION = 1;
const MAX_HEADER_BYTES = 65536;
const MAX_PAYLOAD_BYTES = 1048576;
const U32_MAX = 0xffffffff;
const TOOL_DECISION_TIMEOUT_MS = 20_000;
const BLOCK_REASON = "Blocked by pilot capture admission";
const EXIT_PARENT_GONE = 70;
const EXIT_CHANNEL_WRITE = 71;
const EXIT_PROTOCOL = 72;
const IDENTITY = /^[A-Za-z0-9_.:-]{1,128}$/;
const FAULT_CLASSES: ReadonlySet<string> = new Set([
	"authority", "pins", "bootstrap", "protocol", "request_binding", "route", "counting", "usage", "retry",
	"provider_limit_breach", "privacy", "persistence", "deadline", "checker_provenance", "cleanup",
	"transport", "containment", "slot_limit",
]);
const CLOSE_REASONS: ReadonlySet<string> = new Set(["completed", "halted"]);
const EMPTY = new Uint8Array(0);

type Obj = Record<string, unknown>;
type HeaderPair = [string, string];
type FaultCode =
	| "config" | "delegate_import" | "model_selection" | "settings" | "protocol" | "invocation"
	| "service_tier" | "fetch_shape" | "channel_write" | "internal";
type CancelReason = "aborted" | "stream_cancelled";
type ChildMessage =
	| {
			type: "ready";
			api: string;
			delegate: string;
			model: { provider: string; id: string; api: string; max_tokens: number };
			thinking_level: string;
			active_tools: string[];
	  }
	| { type: "fetch_begin"; invocation: number; fetch: number; method: string; url: string; headers: HeaderPair[] }
	| { type: "fetch_cancel"; fetch: number; reason: CancelReason }
	| { type: "tool_check"; call: string; name: string }
	| { type: "tool_done"; call: string; outcome: "ok" | "error" | "blocked"; result_sha256: string; result_bytes: number }
	| { type: "fault"; code: FaultCode };
type ParentMessage =
	| { type: "response_start"; fetch: number; status: number; headers: HeaderPair[] }
	| { type: "response_data"; fetch: number }
	| { type: "response_end"; fetch: number }
	| { type: "response_error"; fetch: number; class: string }
	| { type: "tool_decision"; call: string; allow: boolean }
	| { type: "close"; reason: string };
interface FetchEntry {
	readonly id: number;
	readonly ctrl: AbortController;
	/**
	 * pending: awaiting response_start; streaming: body open; cancelled: the bridge cancelled it and
	 * Rust frames already in flight are dropped until a terminal frame; done: settled by Rust.
	 */
	state: "pending" | "streaming" | "cancelled" | "done";
	readonly resolve: (response: Response) => void;
	readonly reject: (error: unknown) => void;
	controller: ReadableStreamDefaultController<Uint8Array> | null;
	detach: () => void;
}
interface PendingTool {
	readonly resolve: (allow: boolean) => void;
	readonly timer: ReturnType<typeof setTimeout>;
}
interface FrozenModel {
	provider: string;
	id: string;
	api: string;
	maxTokens: number;
}
type Handler = (event: unknown, ctx: unknown) => unknown;
interface PiApi {
	registerProvider(name: string, config: { api: string; streamSimple: typeof streamSimple }): void;
	on(event: string, handler: Handler): void;
	setModel(model: Obj): Promise<boolean>;
	setThinkingLevel(level: string): void;
	getThinkingLevel(): unknown;
	setActiveTools(names: string[]): Promise<void>;
	getActiveTools(): unknown;
	setServiceTier?: (family: string, tier: undefined) => void;
}
interface BunRuntime {
	file(fd: number): { stream(): ReadableStream<Uint8Array> };
}

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
let session = "";
let host = "";
let loaded = false;
let sessionStarted = false;
let outSeq = 0;
let inSeq = 0;
/** No further outbound frames: close received, channel dead or terminal fault. */
let quiet = false;
let closed = false;
let invocationSeq = 0;
let fetchSeq = 0;
let inBuf: Uint8Array = EMPTY;
const controllers = new Set<AbortController>();
const fetches = new Map<number, FetchEntry>();
const pendingTools = new Map<string, PendingTool>();
const expiredTools = new Set<string>();
const blockedTools = new Set<string>();

function isObj(v: unknown): v is Obj {
	return typeof v === "object" && v !== null && !Array.isArray(v);
}
function isPlain(v: unknown): v is Obj {
	if (!isObj(v)) return false;
	const proto: unknown = Object.getPrototypeOf(v);
	return proto === Object.prototype || proto === null;
}
function exactKeys(o: Obj, keys: readonly string[]): boolean {
	const own = Object.keys(o);
	return own.length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
}
function isUint(v: unknown, max: number): v is number {
	return typeof v === "number" && Number.isInteger(v) && v >= 0 && v <= max;
}
function isPairList(v: unknown): v is HeaderPair[] {
	return Array.isArray(v) && v.every((p) => Array.isArray(p) && p.length === 2 && typeof p[0] === "string" && typeof p[1] === "string");
}
function isSignal(v: unknown): v is AbortSignal {
	return isObj(v) && typeof v.aborted === "boolean" && typeof v.addEventListener === "function" && typeof v.removeEventListener === "function";
}
/** The fd is a FIFO whose inode is exactly the one Rust created and named in the environment. */
function isChannel(fd: number, inode: string | undefined): boolean {
	if (inode === undefined || !/^[0-9]{1,20}$/.test(inode)) return false;
	try {
		const st = fs.fstatSync(fd, { bigint: true });
		// The pinned host's Bun reports bigint inodes as signed 64-bit values.
		return st.isFIFO() && BigInt.asUintN(64, BigInt(st.ino)) === BigInt(inode);
	} catch {
		return false;
	}
}
function isFrozenModel(m: unknown): m is FrozenModel {
	return isObj(m) && m.provider === PROVIDER && m.id === MODEL && m.api === API && m.maxTokens === MAX_TOKENS;
}
function abortError(): DOMException {
	return new DOMException("pilot capture refused", "AbortError");
}

// ---- Outbound channel (fd 3) ----

/** Writes one whole frame. False when it would exceed the bounds or the channel is quiet. */
function send(msg: ChildMessage, payload: Uint8Array = EMPTY): boolean {
	if (quiet) return false;
	const header = encoder.encode(JSON.stringify({ v: PROTOCOL_VERSION, session, host, seq: outSeq + 1, msg }));
	if (header.length === 0 || header.length > MAX_HEADER_BYTES || payload.length > MAX_PAYLOAD_BYTES) return false;
	const frame = new Uint8Array(8 + header.length + payload.length);
	const view = new DataView(frame.buffer);
	view.setUint32(0, header.length, true);
	frame.set(header, 4);
	view.setUint32(4 + header.length, payload.length, true);
	frame.set(payload, 8 + header.length);
	let offset = 0;
	try {
		while (offset < frame.length) {
			const written = fs.writeSync(TX_FD, frame, offset, frame.length - offset);
			if (!(written > 0)) throw new Error("channel write made no progress");
			offset += written;
		}
	} catch {
		terminate(EXIT_CHANNEL_WRITE);
	}
	outSeq += 1;
	return true;
}

function fault(code: FaultCode): void {
	send({ type: "fault", code });
}

/** Settles every in-flight fetch and tool decision locally; `everyInvocation` also aborts idle delegates. */
function abortInFlight(everyInvocation: boolean): void {
	if (everyInvocation) for (const ctrl of controllers) ctrl.abort();
	for (const entry of fetches.values()) {
		if (entry.state !== "pending" && entry.state !== "streaming") continue;
		entry.ctrl.abort();
		settleLocally(entry);
	}
	fetches.clear();
	for (const pending of pendingTools.values()) {
		clearTimeout(pending.timer);
		pending.resolve(false);
	}
	pendingTools.clear();
}

function terminate(exitCode: number): never {
	quiet = true;
	abortInFlight(true);
	process.exit(exitCode);
}

function protocolFault(): never {
	fault("protocol");
	terminate(EXIT_PROTOCOL);
}

// ---- Fetch relay state ----

function errorStream(entry: FetchEntry): void {
	try {
		entry.controller?.error(abortError());
	} catch {
		// The consumer already closed or cancelled the body.
	}
}

function settleLocally(entry: FetchEntry): void {
	const previous = entry.state;
	if (previous !== "pending" && previous !== "streaming") return;
	entry.state = "cancelled";
	entry.detach();
	if (previous === "pending") entry.reject(abortError());
	else errorStream(entry);
}

/** The native side abandoned the fetch: tell Rust once, then error the native request. */
function cancelFetch(entry: FetchEntry, reason: CancelReason): void {
	const previous = entry.state;
	if (previous !== "pending" && previous !== "streaming") return;
	entry.state = "cancelled";
	entry.detach();
	send({ type: "fetch_cancel", fetch: entry.id, reason });
	if (previous === "pending") entry.reject(abortError());
	else if (reason === "aborted") errorStream(entry);
}

function knownFetch(id: number): FetchEntry {
	const entry = fetches.get(id);
	if (entry === undefined) protocolFault();
	return entry;
}

function onResponseStart(id: number, status: number, pairs: HeaderPair[]): void {
	const entry = knownFetch(id);
	if (entry.state === "cancelled") return;
	// Rust relays only successful responses; any other status would trigger host dumps and retries.
	if (entry.state !== "pending" || status < 200 || status > 299) protocolFault();
	let response: Response;
	try {
		const headers = new Headers();
		for (const [name, value] of pairs) headers.append(name, value);
		const body = new ReadableStream<Uint8Array>({
			start: (controller) => {
				entry.controller = controller;
			},
			cancel: () => {
				cancelFetch(entry, "stream_cancelled");
			},
		});
		response = new Response(body, { status, headers });
	} catch {
		protocolFault();
	}
	entry.state = "streaming";
	entry.resolve(response);
}

function onResponseData(id: number, payload: Uint8Array): void {
	const entry = knownFetch(id);
	if (entry.state === "cancelled") return;
	if (entry.state !== "streaming" || entry.controller === null) protocolFault();
	entry.controller.enqueue(payload);
}

function onResponseEnd(id: number): void {
	const entry = knownFetch(id);
	fetches.delete(id);
	if (entry.state === "cancelled") return;
	if (entry.state !== "streaming" || entry.controller === null) protocolFault();
	entry.state = "done";
	entry.detach();
	entry.controller.close();
}

function onResponseError(id: number): void {
	const entry = knownFetch(id);
	fetches.delete(id);
	if (entry.state === "cancelled") return;
	const wasStreaming = entry.state === "streaming";
	entry.state = "done";
	entry.detach();
	// Abort first: OMP's provider loop and client never retry a caller abort.
	entry.ctrl.abort();
	if (wasStreaming) errorStream(entry);
	else entry.reject(abortError());
}

function onToolDecision(call: string, allow: boolean): void {
	const pending = pendingTools.get(call);
	if (pending === undefined) {
		if (expiredTools.delete(call)) return;
		protocolFault();
	}
	pendingTools.delete(call);
	clearTimeout(pending.timer);
	pending.resolve(allow);
}

// ---- Inbound channel (fd 4) ----

function parseHeader(bytes: Uint8Array): ParentMessage {
	let header: unknown;
	try {
		header = JSON.parse(decoder.decode(bytes));
	} catch {
		protocolFault();
	}
	if (!isObj(header) || !exactKeys(header, ["v", "session", "host", "seq", "msg"])) protocolFault();
	if (header.v !== PROTOCOL_VERSION || header.session !== session || header.host !== host || header.seq !== inSeq + 1) {
		protocolFault();
	}
	inSeq += 1;
	const m = header.msg;
	if (!isObj(m)) protocolFault();
	switch (m.type) {
		case "response_start":
			if (exactKeys(m, ["type", "fetch", "status", "headers"]) && isUint(m.fetch, U32_MAX) && isUint(m.status, 0xffff) && isPairList(m.headers)) {
				return { type: "response_start", fetch: m.fetch, status: m.status, headers: m.headers };
			}
			break;
		case "response_data":
			if (exactKeys(m, ["type", "fetch"]) && isUint(m.fetch, U32_MAX)) return { type: "response_data", fetch: m.fetch };
			break;
		case "response_end":
			if (exactKeys(m, ["type", "fetch"]) && isUint(m.fetch, U32_MAX)) return { type: "response_end", fetch: m.fetch };
			break;
		case "response_error":
			if (exactKeys(m, ["type", "fetch", "class"]) && isUint(m.fetch, U32_MAX) && typeof m.class === "string" && FAULT_CLASSES.has(m.class)) {
				return { type: "response_error", fetch: m.fetch, class: m.class };
			}
			break;
		case "tool_decision":
			if (exactKeys(m, ["type", "call", "allow"]) && typeof m.call === "string" && typeof m.allow === "boolean") {
				return { type: "tool_decision", call: m.call, allow: m.allow };
			}
			break;
		case "close":
			if (exactKeys(m, ["type", "reason"]) && typeof m.reason === "string" && CLOSE_REASONS.has(m.reason)) {
				return { type: "close", reason: m.reason };
			}
			break;
	}
	protocolFault();
}

function handleFrame(headerBytes: Uint8Array, payload: Uint8Array): void {
	if (closed) protocolFault();
	const msg = parseHeader(headerBytes);
	if ((msg.type === "response_data") !== payload.length > 0) protocolFault();
	switch (msg.type) {
		case "response_start":
			return onResponseStart(msg.fetch, msg.status, msg.headers);
		case "response_data":
			return onResponseData(msg.fetch, payload);
		case "response_end":
			return onResponseEnd(msg.fetch);
		case "response_error":
			return onResponseError(msg.fetch);
		case "tool_decision":
			return onToolDecision(msg.call, msg.allow);
		case "close":
			closed = true;
			quiet = true;
			abortInFlight(false);
			return;
	}
}

function onChunk(chunk: Uint8Array): void {
	if (inBuf.length === 0) {
		inBuf = new Uint8Array(chunk);
	} else {
		const merged = new Uint8Array(inBuf.length + chunk.length);
		merged.set(inBuf, 0);
		merged.set(chunk, inBuf.length);
		inBuf = merged;
	}
	let offset = 0;
	while (inBuf.length - offset >= 4) {
		const view = new DataView(inBuf.buffer, inBuf.byteOffset + offset, inBuf.length - offset);
		const headerLen = view.getUint32(0, true);
		if (headerLen === 0 || headerLen > MAX_HEADER_BYTES) protocolFault();
		if (view.byteLength < 8 + headerLen) break;
		const payloadLen = view.getUint32(4 + headerLen, true);
		if (payloadLen > MAX_PAYLOAD_BYTES) protocolFault();
		const total = 8 + headerLen + payloadLen;
		if (view.byteLength < total) break;
		const headerBytes = inBuf.subarray(offset + 4, offset + 4 + headerLen);
		const payload = inBuf.slice(offset + 8 + headerLen, offset + total);
		offset += total;
		handleFrame(headerBytes, payload);
	}
	if (offset > 0) inBuf = inBuf.slice(offset);
}

function startReader(bun: BunRuntime): void {
	const reader = bun.file(RX_FD).stream().getReader();
	const pump = async (): Promise<void> => {
		for (;;) {
			let result: ReadableStreamReadResult<Uint8Array>;
			try {
				result = await reader.read();
			} catch {
				result = { done: true, value: undefined };
			}
			if (result.done) {
				// After close, EOF is the normal end; before it, the parent is gone.
				if (!closed) terminate(EXIT_PARENT_GONE);
				return;
			}
			try {
				onChunk(result.value);
			} catch {
				fault("internal");
				terminate(EXIT_PROTOCOL);
			}
		}
	};
	void pump();
}

// ---- Custom API handler ----

function toHeaderPairs(headers: unknown): HeaderPair[] | null {
	const out: HeaderPair[] = [];
	if (headers === undefined || headers === null) return out;
	if (headers instanceof Headers) {
		headers.forEach((value, name) => {
			out.push([name, value]);
		});
		return out;
	}
	if (Array.isArray(headers)) {
		for (const pair of headers) {
			if (!Array.isArray(pair) || pair.length !== 2) return null;
			out.push([String(pair[0]), String(pair[1])]);
		}
		return out;
	}
	if (!isPlain(headers)) return null;
	for (const [name, value] of Object.entries(headers)) out.push([name, String(value)]);
	return out;
}

function toBodyBytes(body: unknown): Uint8Array | null {
	if (typeof body === "string") return encoder.encode(body);
	if (body instanceof ArrayBuffer) return new Uint8Array(body);
	if (ArrayBuffer.isView(body)) return new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
	return null;
}

function relayFetchFor(invocation: number, ctrl: AbortController): (input: unknown, init?: unknown) => Promise<Response> {
	return async (input: unknown, init?: unknown): Promise<Response> => {
		const id = ++fetchSeq;
		const refuse = (): never => {
			fault("fetch_shape");
			ctrl.abort();
			throw abortError();
		};
		if (quiet) {
			ctrl.abort();
			throw abortError();
		}
		const url = typeof input === "string" ? input : input instanceof URL ? input.href : refuse();
		if (!isPlain(init)) return refuse();
		const headers = toHeaderPairs(init.headers);
		const body = toBodyBytes(init.body);
		const signal = init.signal;
		if (headers === null || body === null || body.length === 0) return refuse();
		if (signal !== undefined && signal !== null && !isSignal(signal)) return refuse();
		// An already aborted request is never relayed (native fetch semantics).
		if (ctrl.signal.aborted || (isSignal(signal) && signal.aborted)) throw abortError();
		const method = String(init.method ?? "GET");
		if (!send({ type: "fetch_begin", invocation, fetch: id, method, url, headers }, body)) return refuse();
		return new Promise<Response>((resolve, reject) => {
			const entry: FetchEntry = { id, ctrl, state: "pending", resolve, reject, controller: null, detach: () => undefined };
			fetches.set(id, entry);
			if (isSignal(signal)) {
				const onAbort = (): void => cancelFetch(entry, "aborted");
				signal.addEventListener("abort", onAbort, { once: true });
				entry.detach = () => signal.removeEventListener("abort", onAbort);
			}
		});
	};
}

async function refusingFetch(): Promise<Response> {
	throw abortError();
}

function streamSimple(model: unknown, context: unknown, options?: unknown): unknown {
	invocationSeq += 1;
	const invocation = invocationSeq;
	const ctrl = new AbortController();
	controllers.add(ctrl);
	const opts: Obj = isObj(options) ? options : {};
	const hostSignal = opts.signal;
	if (isSignal(hostSignal)) {
		if (hostSignal.aborted) ctrl.abort();
		else hostSignal.addEventListener("abort", () => ctrl.abort(), { once: true });
	}
	const delegateModel = { ...(isObj(model) ? model : {}), api: DELEGATE_API };
	if (!isFrozenModel(model)) {
		fault("invocation");
		ctrl.abort();
		return streamAnthropic(delegateModel, context, { signal: ctrl.signal, apiKey: SYNTHETIC_KEY, isOAuth: false, fetch: refusingFetch });
	}
	const onPayload = async (payload: unknown, payloadModel: unknown): Promise<unknown> => {
		const hostOnPayload = opts.onPayload;
		const replaced: unknown = typeof hostOnPayload === "function" ? await Reflect.apply(hostOnPayload, opts, [payload, payloadModel]) : undefined;
		const p = replaced === undefined ? payload : replaced;
		if (!isObj(p) || "speed" in p || (p.service_tier !== undefined && p.service_tier !== SERVICE_TIER)) {
			fault("service_tier");
			ctrl.abort();
			throw abortError();
		}
		p.service_tier = SERVICE_TIER;
		return p;
	};
	return streamAnthropic(delegateModel, context, {
		signal: ctrl.signal,
		apiKey: SYNTHETIC_KEY,
		isOAuth: false,
		maxTokens: MAX_TOKENS,
		thinkingEnabled: true,
		effort: EFFORT,
		cacheRetention: "short",
		anthropicPrefixMismatchBehavior: "error",
		sessionId: opts.sessionId,
		providerSessionState: opts.providerSessionState,
		onPayload,
		onResponse: opts.onResponse,
		fetch: relayFetchFor(invocation, ctrl),
		acceptEmptyResponse: true,
		loopGuard: { enabled: false },
		providerRetryWait: async (): Promise<never> => {
			ctrl.abort();
			throw abortError();
		},
	});
}

// ---- Native hooks ----

function observedModel(ctx: Obj): FrozenModel | null {
	if (isFrozenModel(ctx.model)) return ctx.model;
	const models = ctx.models as { current?: () => unknown } | undefined;
	let current: unknown;
	try {
		current = models?.current?.();
	} catch {
		current = undefined;
	}
	return isFrozenModel(current) ? current : null;
}

function exactTools(tools: unknown): tools is string[] {
	return (
		Array.isArray(tools) &&
		tools.length === TOOL_NAMES.length &&
		tools.every((t) => typeof t === "string") &&
		TOOL_NAMES.every((name) => tools.includes(name))
	);
}

async function onSessionStart(pi: PiApi, ctxValue: unknown): Promise<void> {
	if (sessionStarted) {
		fault("internal");
		return;
	}
	sessionStarted = true;
	const ctx: Obj = isObj(ctxValue) ? ctxValue : {};
	let stage: FaultCode = "model_selection";
	try {
		const registry = ctx.modelRegistry as { find?: (provider: string, id: string) => unknown } | undefined;
		const found = registry?.find?.(PROVIDER, MODEL);
		if (!isObj(found)) {
			fault("model_selection");
			return;
		}
		const ok = await pi.setModel({ ...found, api: API, maxTokens: MAX_TOKENS });
		stage = "settings";
		pi.setThinkingLevel(THINKING_LEVEL);
		await pi.setActiveTools([...TOOL_NAMES]);
		if (typeof pi.setServiceTier === "function") pi.setServiceTier(PROVIDER, undefined);
		stage = "model_selection";
		const model = observedModel(ctx);
		if (ok !== true || model === null) {
			fault("model_selection");
			return;
		}
		stage = "settings";
		const thinking = pi.getThinkingLevel();
		const tools = pi.getActiveTools();
		if (thinking !== THINKING_LEVEL || !exactTools(tools)) {
			fault("settings");
			return;
		}
		const ready: ChildMessage = {
			type: "ready",
			api: API,
			delegate: typeof streamAnthropic,
			model: { provider: model.provider, id: model.id, api: model.api, max_tokens: model.maxTokens },
			thinking_level: thinking,
			active_tools: [...tools],
		};
		if (!send(ready)) fault("internal");
	} catch {
		fault(stage);
	}
}

async function onToolCall(event: unknown): Promise<{ block: true; reason: string } | undefined> {
	const e: Obj = isObj(event) ? event : {};
	const call = e.toolCallId;
	const name = e.toolName;
	const block = { block: true as const, reason: BLOCK_REASON };
	if (typeof call !== "string" || typeof name !== "string") {
		fault("internal");
		return block;
	}
	if (pendingTools.has(call)) protocolFault();
	let args: string | undefined;
	try {
		args = JSON.stringify(e.input ?? null);
	} catch {
		args = undefined;
	}
	if (quiet || typeof args !== "string" || args.length === 0 || !send({ type: "tool_check", call, name }, encoder.encode(args))) {
		if (!quiet) fault("internal");
		blockedTools.add(call);
		return block;
	}
	const allow = await new Promise<boolean>((resolve) => {
		const timer = setTimeout(() => {
			pendingTools.delete(call);
			expiredTools.add(call);
			resolve(false);
		}, TOOL_DECISION_TIMEOUT_MS);
		pendingTools.set(call, { resolve, timer });
	});
	if (allow) return undefined;
	blockedTools.add(call);
	return block;
}

function onToolResult(event: unknown): void {
	const e: Obj = isObj(event) ? event : {};
	const call = e.toolCallId;
	if (typeof call !== "string") {
		fault("internal");
		return;
	}
	let bytes: Uint8Array;
	try {
		const blocks: unknown[] = Array.isArray(e.content) ? e.content : [];
		const parts = blocks.map((b) => (isObj(b) && b.type === "text" && typeof b.text === "string" ? b.text : JSON.stringify(b)));
		bytes = encoder.encode(parts.join("\n"));
	} catch {
		fault("internal");
		return;
	}
	const outcome = blockedTools.has(call) ? "blocked" : e.isError ? "error" : "ok";
	blockedTools.delete(call);
	const result_sha256 = createHash("sha256").update(bytes).digest("hex");
	if (!send({ type: "tool_done", call, outcome, result_sha256, result_bytes: bytes.length })) fault("internal");
}

export default function capture(pi: PiApi): void {
	if (loaded) throw new Error("pilot capture bridge: loaded twice");
	loaded = true;
	const env = process.env;
	const sessionId = env.CF_PILOT_SESSION ?? "";
	const hostId = env.CF_PILOT_HOST ?? "";
	// Without valid identities and a FIFO on fd 3 nothing can be reported safely.
	if (!IDENTITY.test(sessionId) || !IDENTITY.test(hostId) || env.CF_PILOT_TX_FD !== String(TX_FD) || !isChannel(TX_FD, env.CF_PILOT_TX_INO)) {
		throw new Error("pilot capture bridge: invalid channel configuration");
	}
	session = sessionId;
	host = hostId;
	const bun: unknown = (globalThis as { Bun?: unknown }).Bun;
	const bunOk = isObj(bun) && typeof bun.file === "function";
	if (env.CF_PILOT_RX_FD !== String(RX_FD) || !isChannel(RX_FD, env.CF_PILOT_RX_INO) || env.CF_PILOT_API !== API || !bunOk) {
		fault("config");
		throw new Error("pilot capture bridge: invalid configuration");
	}
	if (typeof streamAnthropic !== "function") {
		fault("delegate_import");
		throw new Error("pilot capture bridge: host delegate unavailable");
	}
	try {
		startReader(bun as unknown as BunRuntime);
	} catch {
		fault("config");
		throw new Error("pilot capture bridge: private channel unreadable");
	}
	pi.registerProvider(PROVIDER, { api: API, streamSimple });
	pi.on("session_start", (_event, ctx) => onSessionStart(pi, ctx));
	pi.on("tool_call", (event) => onToolCall(event));
	pi.on("tool_result", (event) => {
		onToolResult(event);
		return undefined;
	});
	pi.on("cache_warming_decision", () => ({ action: "stop" }));
}
