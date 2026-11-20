// Test-only observation of one pinned native registration, never a product barrier.
const resolverDescriptor = Object.getOwnPropertyDescriptor(Promise.prototype, "catch")!;
const resolverCatch = resolverDescriptor.value;
const resolverThen = Promise.prototype.then;
const resolverRows: Record<string, unknown>[] = [];
const resolverRefs: { source: Promise<unknown>; derived: Promise<unknown> }[] = [];
let resolverSeedSource: Promise<unknown>;
let resolverSeedResolve!: () => void;
let resolverSeedReject!: () => void;
const resolverSeedCallback = () => null;
const resolverState = {
	installed: false, restored: false, overflow: false,
	seed_receiver_exact: false, seed_return_exact: false,
};
Object.defineProperty(Promise.prototype, "catch", {
	...resolverDescriptor,
	value: function (this: Promise<unknown>, ...args: unknown[]) {
		const derived = Reflect.apply(resolverCatch, this, args);
		if (typeof args[0] !== "function") return derived;
		const callback = Function.prototype.toString.call(args[0]);
		if (callback.length > 256 || callback.replace(/\s/g, "") !== "()=>null") return derived;
		const seed = args[0] === resolverSeedCallback;
		const stack = new Error().stack ?? "";
		if (!seed && !stack.includes("\n    at aee (/$bunfs/root/chunks/cli-3yasrrk4.js:87:22)")) return derived;
		if (resolverRows.length >= 2 || stack.length > 2048) {
			resolverState.overflow = true;
			return derived;
		}
		const row = {
			seed, source_is_promise: this instanceof Promise, derived_is_promise: derived instanceof Promise,
			source_fulfilled: false, source_rejected: false, derived_fulfilled: false, derived_rejected: false,
		};
		resolverRows.push(row);
		resolverRefs.push({ source: this, derived });
		if (seed) resolverState.seed_receiver_exact = this === resolverSeedSource;
		Reflect.apply(resolverThen, this, [() => { row.source_fulfilled = true; }, () => { row.source_rejected = true; }]);
		Reflect.apply(resolverThen, derived, [() => { row.derived_fulfilled = true; }, () => { row.derived_rejected = true; }]);
		return derived;
	},
});
resolverState.installed = Promise.prototype.catch !== resolverCatch;
resolverSeedSource = new Promise<void>((resolve, reject) => {
	resolverSeedResolve = resolve;
	resolverSeedReject = () => reject(new Error("synthetic resolver rejection"));
});
const resolverSeedDerived = resolverSeedSource.catch(resolverSeedCallback);
resolverState.seed_return_exact = resolverRefs[0]?.derived === resolverSeedDerived;
const resolverSnapshot = () => resolverRows.map(row => ({ ...row }));
const resolverOwner = {
	state: resolverState,
	snapshot: resolverSnapshot,
	restore() {
		Object.defineProperty(Promise.prototype, "catch", resolverDescriptor);
		const restored = Object.getOwnPropertyDescriptor(Promise.prototype, "catch")!;
		resolverState.restored = restored.value === resolverCatch && restored.writable === resolverDescriptor.writable &&
			restored.configurable === resolverDescriptor.configurable && restored.enumerable === resolverDescriptor.enumerable;
	},
	resolveSeed: resolverSeedResolve,
	rejectSeed: resolverSeedReject,
	async join(seed: boolean) {
		const indices = resolverRows.flatMap((row, index) => row.seed === seed ? [index] : []);
		if (indices.length !== 1 || resolverState.overflow || !resolverState.restored) throw new Error("resolver identity refused");
		const pair = resolverRefs[indices[0]];
		try {
			// Await the exact original receiver AND exact original catch return.
			// A recovered derived promise alone would conceal source rejection.
			await Promise.all([pair.source, pair.derived]);
			return true;
		} catch {
			return false;
		}
	},
};
Object.defineProperty(globalThis, Symbol.for("cf-native-resolver"), { value: resolverOwner });
process.once("exit", () => {
	const data = JSON.stringify({ ...resolverState, rows: resolverSnapshot() });
	if (Buffer.byteLength(data) > 16384) throw new Error("resolver result cap");
	fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT!, data, { flag: "wx", mode: 0o600 });
});
