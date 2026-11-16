// Owned qualifier only, pinned to OMP 18.18.1. The trusted factory runs before
// createAgentSession starts this original call; session_start seals its result.
// Fixed-profile retirement stops the original listener and closes its native store.
// No credential values are read; physical FD release/general OAuth drain are not claimed.
export async function installOwnedPrefetchGuard() {
	const { AuthStorage, SqliteAuthCredentialStore } = await import("@oh-my-pi/pi-ai");
	const lifecycle = (globalThis as unknown as Record<symbol, {
		bind(): void; retire(closeOriginal: () => void): Promise<unknown>;
	}>)[Symbol.for("context-foundry.qualification.auth")];
	if (!lifecycle || typeof lifecycle.bind !== "function" || typeof lifecycle.retire !== "function") throw new Error("owned auth lifecycle unavailable");
	lifecycle.bind();
	const descriptor = Object.getOwnPropertyDescriptor(AuthStorage.prototype, "keys");
	if (typeof descriptor?.get !== "function") throw new Error("owned prefetch getter unavailable");
	const originalGet = descriptor.get;
	let ownerPrototype: object | undefined;
	let receiver: unknown;
	let active: unknown;
	let sealed = false;
	let failed = false;
	let calls = 0;
	let completed = 0;
	let pending = false;
	let rejected = false;
	let sourceChecks = 0;
	let nonOauthChecks = 0;
	let retirementStarted = false;
	function refuse(): never {
		failed = true;
		if (sealed) process.exit(75);
		throw new Error("owned prefetch refused");
	}
	Object.defineProperty(AuthStorage.prototype, "keys", {
		enumerable: descriptor.enumerable,
		configurable: false,
		get: function () {
			const keys = Reflect.apply(originalGet, this, []);
			if (keys === null || typeof keys !== "object") return refuse();
			const prototype = Object.getPrototypeOf(keys);
			if (ownerPrototype !== undefined && ownerPrototype !== prototype) return refuse();
			if (ownerPrototype === undefined) {
				const prefetch = Object.getOwnPropertyDescriptor(prototype, "prefetchUsage");
				const source = Object.getOwnPropertyDescriptor(prototype, "source");
				if (typeof prefetch?.value !== "function" || typeof source?.value !== "function") return refuse();
				const originalPrefetch = prefetch.value;
				const originalSource = source.value;
				Object.defineProperty(prototype, "source", {
					...source, configurable: false, writable: false,
					value: function (...args: unknown[]) {
						const metadata = Reflect.apply(originalSource, this, args);
						if (active === this) {
							if (++sourceChecks !== 1 || args[0] !== "anthropic") return refuse();
							// Undefined is the SDK's explicit classification for no configured
							// auth. Other unknown shapes/kinds cannot establish non-OAuth.
							if (metadata !== undefined && (!metadata || typeof metadata !== "object" ||
								!["runtime", "config", "api_key", "env"].includes(metadata.kind))) return refuse();
							nonOauthChecks++;
						}
						return metadata;
					},
				});
				Object.defineProperty(prototype, "prefetchUsage", {
					...prefetch, configurable: false, writable: false,
					// Not async: the caller receives the exact original promise.
					value: function (...args: unknown[]) {
						if (sealed || failed || calls !== 0 || args[0] !== "anthropic") return refuse();
						calls = 1;
						receiver = this;
						active = this;
						pending = true;
						let promise;
						try { promise = Reflect.apply(originalPrefetch, this, args); }
						catch { pending = false; rejected = true; return refuse(); }
						finally { active = undefined; }
						if (!(promise instanceof Promise)) return refuse();
						promise.then(() => { completed++; pending = false; }, () => { rejected = true; pending = false; });
						return promise;
					},
				});
				ownerPrototype = prototype;
			}
			return keys;
		},
	});
	return Object.freeze({
		seal(keys: unknown) {
			if (sealed || failed || receiver !== keys || calls !== 1 || completed !== 1 ||
				pending || rejected || sourceChecks !== 1 || nonOauthChecks !== 1) return refuse();
			sealed = true;
			return Object.freeze({ calls, completed, source_checks: sourceChecks, non_oauth_checks: nonOauthChecks });
		},
		async retire(auth: unknown) {
			if (!sealed || failed || retirementStarted || !(auth instanceof AuthStorage) || auth.keys !== receiver) return refuse();
			retirementStarted = true;
			return lifecycle.retire(() => {
				const close = Object.getOwnPropertyDescriptor(SqliteAuthCredentialStore.prototype, "close");
				const poll = Object.getOwnPropertyDescriptor(SqliteAuthCredentialStore.prototype, "pollExternalChanges");
				const authClose = Object.getOwnPropertyDescriptor(AuthStorage.prototype, "close");
				if (typeof close?.value !== "function" || !close.configurable || typeof poll?.value !== "function" || typeof authClose?.value !== "function") return refuse();
				let closing = false, storeCalls = 0, closedWrapper = false;
				Object.defineProperty(SqliteAuthCredentialStore.prototype, "close", { ...close, value: function (...args: unknown[]) {
					if (!closing) return Reflect.apply(close.value, this, args);
					if (++storeCalls !== 1 || !(this instanceof SqliteAuthCredentialStore) || args.length !== 0 ||
						typeof Reflect.apply(poll.value, this, []) !== "boolean") return refuse();
					const returned = Reflect.apply(close.value, this, args);
					if (returned !== undefined) return refuse();
					// Version/revision queries only; never read credential rows.
					try { Reflect.apply(poll.value, this, []); } catch { closedWrapper = true; }
					return returned;
				} });
				let returned;
				try { closing = true; returned = Reflect.apply(authClose.value, auth, []); }
				finally { closing = false; Object.defineProperty(SqliteAuthCredentialStore.prototype, "close", close); }
				if (returned !== undefined || storeCalls !== 1 || !closedWrapper) return refuse();
			});
		},
	});
}
