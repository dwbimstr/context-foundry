/**
 * Metadata-only diagnostic for the artifact-pinned, no-provider qualifier.
 * This file grants no admission. Its trace is not the capture journal, and
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
	if (sequence > 22 || bytes > 8192) fail(74);
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
			throw new Error("CF_QUALIFICATION_STARTUP_REFUSED");
		},
	});
}

record({ kind: "installed", runtime: Bun.version, workspace_restored: true });
process.on("exit", code => {
	record({ kind: "exit", code });
	fs.closeSync(fd);
});
