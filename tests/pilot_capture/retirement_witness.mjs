// Actual original auth owner; the active case deliberately leaves it registered.
import * as fs from 'node:fs';
import {getBundledModel} from '@oh-my-pi/pi-catalog/models';
import {AuthStorage} from '@oh-my-pi/pi-ai';
import {Settings} from '@oh-my-pi/pi-coding-agent/config/settings';
import {lookup} from '@oh-my-pi/pi-coding-agent/config/registry';
import {installOwnedPrefetchGuard} from './startup_auth.ts';
export default async function retirementWitness(pi){
 const kind=process.env.CF_STARTUP_WITNESS_KIND?.replace('retirement-','');
 if(!['active','retire','queued'].includes(kind))throw new Error('retirement control kind');
 const guard=await installOwnedPrefetchGuard(),settings=Settings.instance,setting=lookup('retry.usageReservePct');
 if(!setting||settings.getAgentDir()!==process.env.PI_CODING_AGENT_DIR)throw new Error('retirement settings owner');
 const state={kind,policy_calls:[],pairs:[],actual_receiver:false,restored:false},refs=[],receivers=[];
 const thenDescriptor=Object.getOwnPropertyDescriptor(Promise.prototype,'then'),originalThen=thenDescriptor.value;
 const policyDescriptor=Object.getOwnPropertyDescriptor(AuthStorage.prototype,'setAccountPolicies'),originalPolicy=policyDescriptor.value;
 const hash=x=>new Bun.CryptoHasher('sha256').update(x).digest('hex');
 Object.defineProperty(Promise.prototype,'then',{...thenDescriptor,value:function(...args){
  const returned=Reflect.apply(originalThen,this,args);
  if(typeof args[0]!=='function')return returned;
  const body=Function.prototype.toString.call(args[0]);
  if(body.length>256||hash(body)!=='73e10fcea65a780ac7077e5857e780e8442086c285b36d9c75c345e4e634602c'||!(new Error().stack??'').includes('at <anonymous> (/$bunfs/root/chunks/cli-3yasrrk4.js:118:15)'))return returned;
  if(refs.length>=2)throw new Error('retirement control pair cap');
  const row={source_fulfilled:false,return_fulfilled:false,rejected:false};state.pairs.push(row);refs.push({source:this,returned});
  Reflect.apply(originalThen,this,[()=>{row.source_fulfilled=true},()=>{row.rejected=true}]);
  Reflect.apply(originalThen,returned,[()=>{row.return_fulfilled=true},()=>{row.rejected=true}]);return returned;
 }});
 Object.defineProperty(AuthStorage.prototype,'setAccountPolicies',{...policyDescriptor,value:function(...args){
  if(state.policy_calls.length>=2)throw new Error('retirement control policy cap');
  state.policy_calls.push(args[0]?.defaultReservePct);receivers.push(this);return Reflect.apply(originalPolicy,this,args);
 }});
 const immediate=()=>new Promise(resolve=>setImmediate(resolve));
 const join=async()=>{await Promise.resolve();await Promise.all(refs.flatMap(x=>[x.source,x.returned]));};
 process.once('exit',()=>{const text=JSON.stringify(state);if(Buffer.byteLength(text)>16384)throw new Error('retirement result cap');fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT,text,{flag:'wx',mode:0o600});});
 const serialized=JSON.stringify([getBundledModel('anthropic','claude-sonnet-5-5')]);if(Buffer.byteLength(serialized)>65536)throw new Error('model cap');
 pi.registerProvider('anthropic',{fetchDynamicModels:async()=>JSON.parse(serialized)});
 pi.on('session_start',async(_event,ctx)=>{
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();await immediate();const auth=ctx.modelRegistry.authStorage;
  if(!(auth instanceof AuthStorage)||setting.get(settings)!==10||settings.hasRuntimeOverride(setting))throw new Error('retirement original owner');
  guard.seal(auth.keys);
  try{
   if(kind==='active'){
    setting.override(settings,11);await join();setting.clearOverride(settings);await join();
   }else{
    if(kind==='queued'){setting.override(settings,11);await Promise.resolve();state.pending_before_remove=refs.length;setting.clearOverride(settings);}
    state.retired=await guard.retire(auth);state.policy_calls_at_retirement=[...state.policy_calls];
    setting.override(settings,12);await immediate();setting.clearOverride(settings);await immediate();
    state.no_future_work=refs.length===(kind==='queued'?2:0)&&state.policy_calls.length===(kind==='queued'?2:0);
   }
   state.actual_receiver=receivers.every(x=>x===auth);state.final_value=setting.get(settings);state.override_after=settings.hasRuntimeOverride(setting);
  }finally{
   Object.defineProperty(Promise.prototype,'then',thenDescriptor);Object.defineProperty(AuthStorage.prototype,'setAccountPolicies',policyDescriptor);
   state.restored=Promise.prototype.then===originalThen&&AuthStorage.prototype.setAccountPolicies===originalPolicy;
  }
 });
}
