// Scoped test observer: preserve native timer handles, callbacks' receivers/args.
const loggingProbe={scope:'none',timers:[],immediates:[],factory:null,session:null,tail:{events:0,unwound:0,disposed:0}};
const realTimeout=globalThis.setTimeout,realClear=globalThis.clearTimeout,realImmediate=globalThis.setImmediate;
const timerHandles=new Map();
globalThis.setTimeout=function(callback,delay,...args){
 if(loggingProbe.scope!=='marker'||delay!==1000)return Reflect.apply(realTimeout,this,[callback,delay,...args]);
 if(loggingProbe.timers.length>=8)process.exit(74);
 const row={created:1,fired:0,cleared:0,pending:true};loggingProbe.timers.push(row);
 const wrapped=function(...values){row.fired++;row.pending=false;return Reflect.apply(callback,this,values);};
 const handle=Reflect.apply(realTimeout,this,[wrapped,delay,...args]);timerHandles.set(handle,row);return handle;
};
globalThis.clearTimeout=function(handle){
 const result=Reflect.apply(realClear,this,[handle]);const row=timerHandles.get(handle);
 if(row){row.cleared++;row.pending=false;}return result;
};
globalThis.setImmediate=function(callback,...args){
 if(loggingProbe.scope!=='transport'&&loggingProbe.scope!=='marker')return Reflect.apply(realImmediate,this,[callback,...args]);
 if(loggingProbe.immediates.length>=8)process.exit(74);
 const row={created:1,fired:0,pending:true};loggingProbe.immediates.push(row);
 const wrapped=function(...values){row.fired++;row.pending=false;return Reflect.apply(callback,this,values);};
 return Reflect.apply(realImmediate,this,[wrapped,...args]);
};
Object.defineProperty(globalThis,Symbol.for('cf-native-logging-probe'),{value:loggingProbe});
process.on('exit',()=>{const data=JSON.stringify(loggingProbe);if(Buffer.byteLength(data)>8192)process.exitCode=74;else fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT,data,{flag:'wx',mode:0o600});});
