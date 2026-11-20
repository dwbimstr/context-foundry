import {getBundledModel} from '@oh-my-pi/pi-catalog/models';
export default async function watchdogWitness(pi){
 const state=globalThis[Symbol.for('cf-native-watchdog')];
 const snapshot=()=>state.rows.map(row=>({pending:row.pending,clear_attempts:row.clear_attempts,native_clears:row.native_clears}));
 state.factory=snapshot();
 if(process.env.CF_STARTUP_WITNESS_KIND==='watchdog-active')await new Promise(resolve=>setTimeout(resolve,11000));
 const serialized=JSON.stringify([getBundledModel('anthropic','claude-sonnet-5-5')]);
 if(Buffer.byteLength(serialized)>65536)throw new Error('watchdog model cap');
 pi.registerProvider('anthropic',{fetchDynamicModels:async()=>JSON.parse(serialized)});
 pi.on('session_start',async(_event,ctx)=>{
  state.session=snapshot();
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
 });
}
