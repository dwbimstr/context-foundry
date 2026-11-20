// Source-owned native control; no Promise fingerprints or copied listener sets.
import * as fs from 'node:fs';
import {getBundledModel} from '@oh-my-pi/pi-catalog/models';
import {AuthStorage} from '@oh-my-pi/pi-ai';
import {Settings} from '@oh-my-pi/pi-coding-agent/config/settings';
import {lookup} from '@oh-my-pi/pi-coding-agent/config/registry';
import {bindCliAuthStorageSettingsSync} from '@oh-my-pi/pi-coding-agent/session/auth-broker-config';
import {installOwnedPrefetchGuard} from './startup_auth.ts';
export default async function sourceAuthWitness(pi){
 const kind=process.env.CF_STARTUP_WITNESS_KIND?.replace('source-retirement-','');
 if(!['active','retire','queued'].includes(kind))throw new Error('source auth control kind');
 const settings=Settings.instance,setting=lookup('retry.usageReservePct');
 let wrong_settings_rejected=false,duplicate_binding_rejected=false;
 try{bindCliAuthStorageSettingsSync(Object.create(Settings.prototype));}catch(error){wrong_settings_rejected=error?.message==='CLI auth owner unavailable';}
 if(!wrong_settings_rejected)throw new Error('source auth identity accepted');
 const guard=await installOwnedPrefetchGuard();
 try{bindCliAuthStorageSettingsSync(settings);}catch(error){duplicate_binding_rejected=error?.message==='CLI auth owner unavailable';}
 if(!duplicate_binding_rejected)throw new Error('source auth binding reused');
 if(!setting||settings.getAgentDir()!==process.env.PI_CODING_AGENT_DIR)throw new Error('source auth settings owner');
 const state={kind,wrong_settings_rejected,duplicate_binding_rejected,policy_calls:[],actual_receiver:false,restored:false},receivers=[];
 const descriptor=Object.getOwnPropertyDescriptor(AuthStorage.prototype,'setAccountPolicies'),original=descriptor.value;
 Object.defineProperty(AuthStorage.prototype,'setAccountPolicies',{...descriptor,value:function(...args){
  if(state.policy_calls.length>=2)throw new Error('source auth policy cap');
  state.policy_calls.push(args[0]?.defaultReservePct);receivers.push(this);return Reflect.apply(original,this,args);
 }});
 const immediate=()=>new Promise(resolve=>setImmediate(resolve));
 process.once('exit',()=>{const text=JSON.stringify(state);if(Buffer.byteLength(text)>16384)throw new Error('source auth result cap');fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT,text,{flag:'wx',mode:0o600});});
 const serialized=JSON.stringify([getBundledModel('anthropic','claude-sonnet-5-5')]);if(Buffer.byteLength(serialized)>65536)throw new Error('model cap');
 pi.registerProvider('anthropic',{fetchDynamicModels:async()=>JSON.parse(serialized)});
 pi.on('session_start',async(_event,ctx)=>{
  await ctx.modelRegistry.awaitInitialBackgroundRefresh();await immediate();const auth=ctx.modelRegistry.authStorage;
  if(!(auth instanceof AuthStorage)||setting.get(settings)!==10||settings.hasRuntimeOverride(setting))throw new Error('source auth original owner');
  guard.seal(auth.keys);
  try{
   if(kind==='active'){
    setting.override(settings,11);await immediate();setting.clearOverride(settings);await immediate();
   }else{
    // Deliver the first callback before stop. The second queued recompute is
    // cancelled by the real Derived unsubscribe; the pending first policy reads 10.
    if(kind==='queued'){setting.override(settings,11);await Promise.resolve();setting.clearOverride(settings);}
    await guard.retire(auth);state.retired=true;state.policy_calls_at_retirement=[...state.policy_calls];
    setting.override(settings,12);await immediate();setting.clearOverride(settings);await immediate();
    state.no_future_work=state.policy_calls.length===(kind==='queued'?1:0);
   }
   state.actual_receiver=receivers.every(x=>x===auth);state.final_value=setting.get(settings);state.override_after=settings.hasRuntimeOverride(setting);
  }finally{
   Object.defineProperty(AuthStorage.prototype,'setAccountPolicies',descriptor);
   state.restored=AuthStorage.prototype.setAccountPolicies===original;
  }
 });
}
