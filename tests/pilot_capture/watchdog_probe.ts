// Test-only observation of the original native interval, callback and handle.
import { createHash } from "node:crypto";
const watchKind = process.env.CF_STARTUP_WITNESS_KIND;
const watchdog = { factory: null as unknown, session: null as unknown, rows: [] as Record<string, unknown>[], other_ten_second_intervals: 0 };
const watchHandles = new Map<unknown, Record<string, unknown>>();
const watchSet = globalThis.setInterval, watchClear = globalThis.clearInterval;
globalThis.setInterval = function (...args: Parameters<typeof setInterval>) {
 const handle = Reflect.apply(watchSet, this, args);
 if (args[1] === 10000 && typeof args[0] === "function") {
  const source = Function.prototype.toString.call(args[0]);
  if (source.includes("Still starting after")) {
   if (watchdog.rows.length || Buffer.byteLength(source) > 4096) throw new Error("watchdog observation cap");
   const row = { native_ms: 10000, callback_preserved: true, arguments_preserved: true,
    source_sha256: createHash("sha256").update(source).digest("hex"), pending: true, clear_attempts: 0, native_clears: 0 };
   watchdog.rows.push(row);watchHandles.set(handle,row);
  } else watchdog.other_ten_second_intervals++;
 }
 return handle;
} as typeof setInterval;
globalThis.clearInterval = function (...args: Parameters<typeof clearInterval>) {
 const row = watchHandles.get(args[0]);
 if (row) {
  row.clear_attempts = Number(row.clear_attempts) + 1;
  if (watchKind === "watchdog-uncleared") return;
 }
 const result = Reflect.apply(watchClear,this,args);
 if (row) { row.native_clears = Number(row.native_clears) + 1;row.pending = false; }
 return result;
};
Object.defineProperty(globalThis,Symbol.for("cf-native-watchdog"),{value:watchdog});
process.once("exit",()=>{
 const serialized=JSON.stringify(watchdog);
 if(Buffer.byteLength(serialized)>8192)throw new Error("watchdog result cap");
 fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT!,serialized,{flag:"wx",mode:0o600});
});
