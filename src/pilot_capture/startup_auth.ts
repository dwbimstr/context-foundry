// Expected-red candidate: an unobserved original prefetch cannot admit capture.
export async function installOwnedPrefetchGuard() {
	return Object.freeze({ seal(_keys: unknown): void { throw new Error("owned prefetch missing"); } });
}
