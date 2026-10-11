import {decodeInventory,type Item} from './inventory.js';
import {at,object,type JsonObject} from './types.js';

export async function extraStorage(member:JsonObject,known:Record<string,Item[]|null>):Promise<JsonObject> {
 const containers:JsonObject[]=[],records:JsonObject[]=[];let scanned=0,truncated=false;
 const record=(path:string,items:Item[]|null,status:string,reasonCode:string|null)=>{
  const viewOnly=/backpack_icons|loadout/.test(path),scope=path.startsWith('shared_inventory.')?'shared_profile':'member';
  containers.push({id:path,scope,realm:path.startsWith('rift.')?'rift':'overworld',viewOnly,status,reasonCode,returned:items?.length??0});
  for(const item of items??[])records.push({...item,location:path,scope,realm:path.startsWith('rift.')?'rift':'overworld',viewOnly});
 };
 for(const [path,items] of Object.entries(known))record(path,items,items===null?'unknown':'known',items===null?'not_exposed_or_decode_failed':null);
 const visit=async(v:unknown,path:string,depth:number):Promise<void>=>{
  if(depth>12||scanned>=64){truncated=true;return;}
  if(typeof at(v,'data')==='string') {
   if(path in known)return;
   scanned++;try{const items=await decodeInventory(v);record(path,items,items===null?'unknown':'known',items===null?'not_exposed':null);}catch{record(path,null,'unknown','invalid_or_unreadable_encoding');}return;
  }
  for(const [k,x] of Object.entries(object(v)))if(/^[A-Za-z0-9_:.-]{1,96}$/.test(k))await visit(x,path?path+'.'+k:k,depth+1);
 };
 for(const root of ['inventory','shared_inventory','rift','foraging'])await visit(member[root],root,0);
 const instances=new Map<string,JsonObject[]>();for(const item of records)if(typeof item.instanceId==='string'){const list=instances.get(item.instanceId)??[];list.push(item);instances.set(item.instanceId,list);}
 const duplicateViews=[...instances].filter(([,items])=>items.length>1).map(([instanceId,items])=>({instanceId,locations:items.map(i=>({location:i.location,slot:i.slot})),countedAsOneInstance:true}));
 return {containers,items:records,duplicateViews,uniqueIdentifiedInstances:instances.size,unidentifiedRecords:records.filter(i=>!i.instanceId).length,coverage:{returnedContainers:containers.length,returnedItemRecords:records.length,truncated,completeOwnedEstate:false,unviewedPlacedOrEmbeddedItemsUnknown:true,anonymousDuplicatesUnresolved:true},materialTotals:null,reasonCode:'totals_unknown_when_location_or_instance_coverage_is_incomplete'};
}
