// Expected-red candidate: unobserved original memory startup cannot admit capture.
export async function installOwnedMemoryGuard() {
	return Object.freeze({ seal(_settings: unknown, _registry: unknown): never { throw new Error("owned memory missing"); } });
}
