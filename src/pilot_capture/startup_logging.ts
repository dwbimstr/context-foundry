// Owned qualifier only. Retire the native file/console transport and join the
// already-queued synchronous retention callback before SDK construction.
import { logger } from "@oh-my-pi/pi-utils";
export async function quiesceOwnedLogging(): Promise<void> {
 const owner = (globalThis as unknown as Record<symbol, { loggingQuiesced?: () => void } | undefined>)[Symbol.for("context-foundry.qualification.warmup")];
 if (typeof owner?.loggingQuiesced !== "function") throw new Error("owned logging owner unavailable");
 logger.setTransports({ console: false, file: false });
 await new Promise<void>(resolve => setImmediate(resolve));
 owner.loggingQuiesced();
}
