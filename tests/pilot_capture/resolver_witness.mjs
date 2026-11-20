import {getBundledModel} from '@oh-my-pi/pi-catalog/models';
import {createAuthStorageSettingsSync} from '@oh-my-pi/pi-coding-agent/session/auth-broker-config';
export default async function resolverWitness(pi){
 const owner=globalThis[Symbol.for('cf-native-resolver')],state=owner.state;
 state.factory=owner.snapshot();owner.restore();
 const body=Function.prototype.toString.call(createAuthStorageSettingsSync);
 if(body.length>16384)throw new Error('native resolver function cap');
 state.native_function_name=createAuthStorageSettingsSync.name;
 state.native_function_sha256=new Bun.CryptoHasher('sha256').update(body).digest('hex');
 if(state.native_function_name!=='aee'||state.native_function_sha256!=='25f681643c58b0b08a43f839ab6760b76ab1cbd65e5ff3ba61d110fff8c03ca9')throw new Error('native resolver source refused');
 state.native_join=await owner.join(false);
 const kind=process.env.CF_STARTUP_WITNESS_KIND;
 if(kind==='resolver-pending'){
  let done=false;
  const pending=owner.join(true).then(value=>{done=true;return value;});
  await Promise.resolve();await Promise.resolve();
  state.pending_control={returned_while_pending:done,before:owner.snapshot()};
  owner.resolveSeed();state.seed_join=await pending;
 }else{
  if(kind==='resolver-rejected')owner.rejectSeed();else owner.resolveSeed();
  state.seed_join=await owner.join(true);
 }
 state.after_join=owner.snapshot();
 const serialized=JSON.stringify([getBundledModel('anthropic','claude-sonnet-5-5')]);
 if(Buffer.byteLength(serialized)>65536)throw new Error('resolver model cap');
 pi.registerProvider('anthropic',{fetchDynamicModels:async()=>JSON.parse(serialized)});
 pi.on('session_start',async(_event,ctx)=>{
  state.session_entry=owner.snapshot();
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();
 });
}
