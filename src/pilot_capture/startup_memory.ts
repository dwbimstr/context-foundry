// The fixed read/edit qualifier admits only the SDK's existing off backend.
// Observe the original call before SDK creation, then seal it at session_start.
export async function installOwnedMemoryGuard() {
	const { offBackend, resolveMemoryBackend } = await import("@oh-my-pi/pi-coding-agent/memory-backend");
	const { Settings } = await import("@oh-my-pi/pi-coding-agent/config/settings");
	const { lookup } = await import("@oh-my-pi/pi-coding-agent/config/registry");
	const settings = Settings.instance;
	let calls = 0, completed = 0;
	let pending = false, rejected = false, failed = false, sealed = false;
	let startSettings: unknown, startRegistry: unknown;
	function refuse(): never {
		failed = true;
		if (sealed) process.exit(75);
		throw new Error("owned memory refused");
	}
	function eligible() {
		if (Settings.instance !== settings || settings.getAgentDir() !== process.env.PI_CODING_AGENT_DIR ||
			lookup("memory.backend")?.get(settings) !== "off" || lookup("autolearn.enabled")?.get(settings) !== false) refuse();
	}
	eligible();
	const descriptor = Object.getOwnPropertyDescriptor(offBackend, "start");
	if (offBackend.id !== "off" || typeof descriptor?.value !== "function") return refuse();
	const original = descriptor.value;
	Object.defineProperty(offBackend, "start", {
		...descriptor, configurable: false, writable: false,
		// Not async: preserve the original receiver, arguments and exact promise.
		value: function (...args: unknown[]) {
			if (sealed || failed || calls !== 0 || this !== offBackend) return refuse();
			calls = 1;
			const context = args[0] as { settings?: unknown; modelRegistry?: unknown } | undefined;
			startSettings = context?.settings;
			startRegistry = context?.modelRegistry;
			pending = true;
			let promise;
			try { promise = Reflect.apply(original, this, args); }
			catch { pending = false; rejected = true; return refuse(); }
			if (!(promise instanceof Promise)) return refuse();
			promise.then(() => { completed++; pending = false; }, () => { rejected = true; pending = false; });
			return promise;
		},
	});
	return Object.freeze({
		async seal(actualSettings: unknown, actualRegistry: unknown) {
			if (actualSettings !== settings || !actualRegistry) return refuse();
			eligible();
			const backend = await resolveMemoryBackend(settings);
			if (sealed || failed || backend !== offBackend || backend.id !== "off" ||
				startSettings !== actualSettings || startRegistry !== actualRegistry || calls !== 1 ||
				completed !== 1 || pending || rejected) return refuse();
			sealed = true;
			return Object.freeze({ calls, completed });
		},
	});
}
