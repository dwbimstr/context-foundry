/** One pipe/sequence owner shared by the early preload and capture extension. */
import * as channelFs from "node:fs";

const CF_CHANNEL_KEY = Symbol.for("context-foundry.capture.channel.v1");
const CF_CHANNEL_TAG = "cf-pilot-private-channel-v1";
const CF_CHANNEL_HEADER_MAX = 65536;
const CF_CHANNEL_PAYLOAD_MAX = 1048576;
const CF_CHANNEL_ID = /^[A-Za-z0-9_.:-]{1,128}$/;
const CF_CHANNEL_EMPTY = new Uint8Array(0);

function channelObject(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
function channelFifo(fd: number, inode: string | undefined): boolean {
	if (inode === undefined || !/^[0-9]{1,20}$/.test(inode)) return false;
	try {
		const metadata = channelFs.fstatSync(fd, { bigint: true });
		return metadata.isFIFO() && BigInt.asUintN(64, BigInt(metadata.ino)) === BigInt(inode);
	} catch { return false; }
}

class OwnedCaptureChannel {
	readonly tag = CF_CHANNEL_TAG;
	#session: string;
	#host: string;
	#txInode: string;
	#rxInode: string;
	#api: string;
	#version: 1 | 2;
	#out = 0;
	#incoming = 0;
	#quiet = false;
	#closed = false;
	#started = false;
	#buffer: Uint8Array = CF_CHANNEL_EMPTY;
	#encoder = new TextEncoder();
	#decoder = new TextDecoder("utf-8", { fatal: true });
	#handler?: (message: unknown, payload: Uint8Array) => void;
	#onFailure?: () => void;
	constructor() {
		const env = process.env;
		this.#session = env.CF_PILOT_SESSION ?? "";
		this.#host = env.CF_PILOT_HOST ?? "";
		this.#txInode = env.CF_PILOT_TX_INO ?? "";
		this.#rxInode = env.CF_PILOT_RX_INO ?? "";
		this.#api = env.CF_PILOT_API ?? "";
		this.#version = this.#api === "cf-pilot-startup-v2" ? 2 : 1;
		if (!CF_CHANNEL_ID.test(this.#session) || !CF_CHANNEL_ID.test(this.#host) ||
						env.CF_PILOT_TX_FD !== "3" || !channelFifo(3, env.CF_PILOT_TX_INO)) {
			throw new Error("pilot capture bridge: invalid channel configuration");
		}
		if (env.CF_PILOT_RX_FD !== "4" || !channelFifo(4, env.CF_PILOT_RX_INO) ||
						(this.#api !== "cf-pilot-capture-v1" && this.#api !== "cf-pilot-startup-v2") || !channelObject((globalThis as { Bun?: unknown }).Bun) || typeof Bun.file !== "function") {
			this.send({ type: "fault", code: "config" });
			throw new Error("pilot capture bridge: invalid configuration");
		}
	}
	matches(): boolean {
		return this.#session === process.env.CF_PILOT_SESSION && this.#host === process.env.CF_PILOT_HOST &&
			process.env.CF_PILOT_TX_FD === "3" && process.env.CF_PILOT_RX_FD === "4" && process.env.CF_PILOT_API === this.#api &&
			process.env.CF_PILOT_TX_INO === this.#txInode && process.env.CF_PILOT_RX_INO === this.#rxInode &&
			channelFifo(3, this.#txInode) && channelFifo(4, this.#rxInode);
	}
	get quiet(): boolean { return this.#quiet; }
	stop(): void { this.#quiet = true; }
	close(): void { this.#closed = true; this.#quiet = true; }
	#terminate(code: number): never {
		this.stop(); this.#onFailure?.(); process.exit(code);
	}
	#protocolFault(): never {
		this.send({ type: "fault", code: "protocol" }); this.#terminate(72);
	}
	send(message: unknown, payload: Uint8Array = CF_CHANNEL_EMPTY): boolean {
		if (this.#quiet || this.#out >= 0xfffffffe || (this.#version === 2 && payload.length)) return false;
		const header = this.#encoder.encode(JSON.stringify({ v: this.#version, session: this.#session, host: this.#host, seq: this.#out + 1, msg: message }));
		if (!header.length || header.length > CF_CHANNEL_HEADER_MAX || payload.length > CF_CHANNEL_PAYLOAD_MAX) return false;
		const frame = new Uint8Array(8 + header.length + payload.length), view = new DataView(frame.buffer);
		view.setUint32(0, header.length, true); frame.set(header, 4);
		view.setUint32(4 + header.length, payload.length, true); frame.set(payload, 8 + header.length);
		try {
			let offset = 0;
			while (offset < frame.length) {
				const written = channelFs.writeSync(3, frame, offset, frame.length - offset);
				if (!(written > 0)) throw new Error("channel write made no progress");
				offset += written;
			}
		} catch { this.#terminate(71); }
		this.#out += 1;
		return true;
	}
	#frame(bytes: Uint8Array, payload: Uint8Array): void {
		if (this.#closed) this.#protocolFault();
		let header: unknown;
		try { header = JSON.parse(this.#decoder.decode(bytes)); } catch { this.#protocolFault(); }
		if (!channelObject(header) || Object.keys(header).length !== 5 ||
						!["v", "session", "host", "seq", "msg"].every(key => Object.prototype.hasOwnProperty.call(header, key)) ||
						header.v !== this.#version || header.session !== this.#session || header.host !== this.#host ||
						header.seq !== this.#incoming + 1 || this.#incoming >= 0xfffffffe || !this.#handler) this.#protocolFault();
		this.#incoming += 1;
		this.#handler(header.msg, payload);
	}
	#chunk(chunk: Uint8Array): void {
		if (!this.#buffer.length) this.#buffer = new Uint8Array(chunk);
		else {
			const merged = new Uint8Array(this.#buffer.length + chunk.length);
			merged.set(this.#buffer); merged.set(chunk, this.#buffer.length); this.#buffer = merged;
		}
		let offset = 0;
		while (this.#buffer.length - offset >= 4) {
			const view = new DataView(this.#buffer.buffer, this.#buffer.byteOffset + offset, this.#buffer.length - offset);
			const header = view.getUint32(0, true);
			if (!header || header > CF_CHANNEL_HEADER_MAX) this.#protocolFault();
			if (view.byteLength < 8 + header) break;
			const payload = view.getUint32(4 + header, true);
			if (payload > (this.#version === 2 ? 0 : CF_CHANNEL_PAYLOAD_MAX)) this.#protocolFault();
			const total = 8 + header + payload;
			if (view.byteLength < total) break;
			const bytes = this.#buffer.subarray(offset + 4, offset + 4 + header);
			const body = this.#buffer.slice(offset + 8 + header, offset + total);
			offset += total; this.#frame(bytes, body);
		}
		if (offset) this.#buffer = this.#buffer.slice(offset);
	}
	attach(handler: (message: unknown, payload: Uint8Array) => void, onFailure: () => void): void {
		if (this.#started || this.#handler || this.#quiet) throw new Error("pilot capture channel attached twice");
		this.#started = true; this.#handler = handler; this.#onFailure = onFailure;
		const reader = Bun.file(4).stream().getReader();
		const pump = async () => {
			for (;;) {
				let item: ReadableStreamReadResult<Uint8Array>;
				try { item = await reader.read(); } catch { item = { done: true, value: undefined }; }
				if (item.done) {
					if (this.#buffer.length) this.#protocolFault();
					if (!this.#closed) this.#terminate(70);
					return;
				}
				try { this.#chunk(item.value); }
				catch { this.send({ type: "fault", code: "internal" }); this.#terminate(72); }
			}
		};
		void pump();
	}
}

/** A second loader gets the same object; identity checks never reset counters. */
export function getCaptureChannel(): OwnedCaptureChannel {
	const globals = globalThis as unknown as Record<symbol, OwnedCaptureChannel | undefined>;
	const existing = globals[CF_CHANNEL_KEY];
	if (existing !== undefined) {
		if (existing.tag !== CF_CHANNEL_TAG || typeof existing.matches !== "function" || !existing.matches()) {
			throw new Error("pilot capture channel owner mismatch");
		}
		return existing;
	}
	const owner = new OwnedCaptureChannel();
	Object.freeze(owner);
	Object.defineProperty(globalThis, CF_CHANNEL_KEY, { value: owner, configurable: false, writable: false });
	return owner;
}
