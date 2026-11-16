// Test-only observation of the original CLI listener; no replacement sync or product barrier.
import * as fs from 'node:fs';
import {getBundledModel} from '@oh-my-pi/pi-catalog/models';
import {AuthStorage} from '@oh-my-pi/pi-ai';
import {Settings} from '@oh-my-pi/pi-coding-agent/config/settings';
import {lookup} from '@oh-my-pi/pi-coding-agent/config/registry';
import {createAuthStorageSettingsSync} from '@oh-my-pi/pi-coding-agent/session/auth-broker-config';
export default async function settingsChainWitness(pi){
 const kind=process.env.CF_STARTUP_WITNESS_KIND?.replace('settings-chain-',''),settings=Settings.instance,setting=lookup('retry.usageReservePct');
 if(!['baseline','serial','coalesced'].includes(kind))throw new Error('settings control kind');
 if(!setting||settings.getAgentDir()!==process.env.PI_CODING_AGENT_DIR)throw new Error('settings owner');
 const hash=value=>new Bun.CryptoHasher('sha256').update(value).digest('hex');
 const state={kind,rows:[],policy_calls:[],before_joins:[],chain_continuity:[],overflow:false,restored:false,actual_auth_receiver:false,native_sync_sha256:hash(Function.prototype.toString.call(createAuthStorageSettingsSync))};
 if(state.native_sync_sha256!=='25f681643c58b0b08a43f839ab6760b76ab1cbd65e5ff3ba61d110fff8c03ca9')throw new Error('native sync source');
 const refs=[],receivers=[];
 const descriptor=Object.getOwnPropertyDescriptor(Promise.prototype,'then'),original=descriptor.value;
 const policyDescriptor=Object.getOwnPropertyDescriptor(AuthStorage.prototype,'setAccountPolicies'),policyOriginal=policyDescriptor.value;
 const snapshot=()=>state.rows.map(row=>({...row}));
 const matches=(target,name,expected)=>{const actual=Object.getOwnPropertyDescriptor(target,name);return actual?.value===expected.value&&actual.writable===expected.writable&&actual.configurable===expected.configurable&&actual.enumerable===expected.enumerable;};
 if(typeof original!=='function'||!descriptor.writable||typeof policyOriginal!=='function'||!policyDescriptor.writable)throw new Error('native settings methods unavailable');
 process.once('exit',()=>{const text=JSON.stringify(state);if(Buffer.byteLength(text)>65536)throw new Error('settings result cap');fs.writeFileSync(process.env.CF_STARTUP_WITNESS_RESULT,text,{flag:'wx',mode:0o600});});
 try{
 Object.defineProperty(Promise.prototype,'then',{...descriptor,value:function(...args){
  const derived=Reflect.apply(original,this,args);
  if(typeof args[0]!=='function')return derived;
  const source=Function.prototype.toString.call(args[0]);if(source.length>256||hash(source)!=='73e10fcea65a780ac7077e5857e780e8442086c285b36d9c75c345e4e634602c')return derived;
  const stack=new Error().stack??'';if(!stack.includes('\n    at <anonymous> (/$bunfs/root/chunks/cli-3yasrrk4.js:118:15)'))return derived;
  if(state.rows.length>=2||stack.length>2048){state.overflow=true;return derived;}
  const row={callback_sha256:hash(source),source_is_promise:this instanceof Promise,derived_is_promise:derived instanceof Promise,source_fulfilled:false,source_rejected:false,derived_fulfilled:false,derived_rejected:false};
  state.chain_continuity.push(refs.length?this===refs[refs.length-1].derived:null);state.rows.push(row);refs.push({source:this,derived});
  Reflect.apply(original,this,[()=>{row.source_fulfilled=true},()=>{row.source_rejected=true}]);
  Reflect.apply(original,derived,[()=>{row.derived_fulfilled=true},()=>{row.derived_rejected=true}]);
  return derived;
 }});
 Object.defineProperty(AuthStorage.prototype,'setAccountPolicies',{...policyDescriptor,value:function(...args){
  if(state.policy_calls.length>=2){state.overflow=true;throw new Error('policy call cap');}
  const row={native_receiver:this instanceof AuthStorage,arguments:args.length,default_reserve_pct:args[0]?.defaultReservePct,return_undefined:false};state.policy_calls.push(row);receivers.push(this);
  const returned=Reflect.apply(policyOriginal,this,args);row.return_undefined=returned===undefined;return returned;
 }});
  state.before_value=setting.get(settings);state.override_before=settings.hasRuntimeOverride(setting);
  if(state.before_value!==10||state.override_before)throw new Error('native reserve profile');
  if(kind!=='baseline'){
  setting.override(settings,state.before_value+1);
  if(kind==='coalesced')setting.override(settings,state.before_value+2);
  for(let i=0;i<2;i++){
   if(i===1)setting.clearOverride(settings);
   await Promise.resolve();
   state.before_joins.push({pair_count:refs.length,policy_calls:state.policy_calls.length,last_derived_fulfilled:state.rows.at(-1)?.derived_fulfilled});
   if(refs.length!==i+1)throw new Error('original pending chain missing');
   await Promise.all([refs[i].source,refs[i].derived]);
  }
  }
  state.after_join=snapshot();state.after_value=setting.get(settings);state.override_after=settings.hasRuntimeOverride(setting);
 }finally{
  Object.defineProperty(Promise.prototype,'then',descriptor);Object.defineProperty(AuthStorage.prototype,'setAccountPolicies',policyDescriptor);
  state.restored=matches(Promise.prototype,'then',descriptor)&&matches(AuthStorage.prototype,'setAccountPolicies',policyDescriptor);
 }
 const serialized=JSON.stringify([getBundledModel('anthropic','claude-sonnet-5-5')]);if(Buffer.byteLength(serialized)>65536)throw new Error('model cap');
 pi.registerProvider('anthropic',{fetchDynamicModels:async()=>JSON.parse(serialized)});
 pi.on('session_start',async(_event,ctx)=>{state.session_entry=snapshot();await ctx.modelRegistry.awaitInitialBackgroundRefresh();const auth=ctx.modelRegistry.authStorage;state.actual_auth_class=auth instanceof AuthStorage;state.actual_auth_receiver=state.actual_auth_class&&receivers.every(x=>x===auth);});
}
