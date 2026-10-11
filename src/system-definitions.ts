import {readFileSync} from 'node:fs';
import {at,number,object,text,type JsonObject} from './types.js';
export const definitions=JSON.parse(readFileSync(new URL('../../fixtures/domain/systems-definitions.json',import.meta.url),'utf8')) as JsonObject;
/** A bounded arithmetic interpreter for pinned reference expressions. No eval, I/O or arbitrary code. */
export function ruleNumber(expression:unknown,variables:Record<string,number>):number|string|null {
 if(typeof expression==='number')return Number.isFinite(expression)?expression:null;
 if(typeof expression!=='string'||expression.length>1024||!expression)return null;
 const tokens=expression.match(/[()]|[^\s()]+/g)??[];let cursor=0,budget=512;
 type Ast=string|Ast[];
 function parse(depth=0):Ast {if(depth>20||--budget<0)throw Error();const token=tokens[cursor++];if(!token)throw Error();if(token==='('){const list:Ast[]=[];while(tokens[cursor]!==')'){if(cursor>=tokens.length)throw Error();list.push(parse(depth+1));}cursor++;return list;}if(token===')')throw Error();return token;}
 function calc(ast:Ast,depth=0):unknown {
  if(--budget<0||depth>20)throw Error();if(typeof ast==='string'){if(/^-?\d+(?:\.\d+)?$/.test(ast))return Number(ast);if(ast in variables)return variables[ast];if(/^[A-Z_]+$/.test(ast))return ast;throw Error();}
  const [op,...args]=ast;if(typeof op!=='string')throw Error();
  if(op==='if'){if(args.length!==3)throw Error();return calc(args[calc(args[0]!,depth+1)?1:2]!,depth+1);}
  const values=args.map(v=>calc(v,depth+1)),ns=values as number[];
  if(op==='list.new')return values;
  if(op==='list.at'){if(!Array.isArray(values[0])||!Number.isInteger(values[1]))throw Error();return values[0][Number(values[1])];}
  if(values.some(v=>typeof v!=='number'||!Number.isFinite(v)))throw Error();
  switch(op){case '+':return ns.reduce((a,b)=>a+b,0);case '*':return ns.reduce((a,b)=>a*b,1);case '-':return ns.length===1?-ns[0]!:ns[0]!-ns[1]!;case '/':return ns[0]!/ns[1]!;case 'pow':return Math.pow(ns[0]!,ns[1]!);case 'floor':return Math.floor(ns[0]!);case 'round':return Math.round(ns[0]!);case 'round-decimals':return Math.round(ns[0]!*10**ns[1]!)/10**ns[1]!;case 'min':return Math.min(...ns);case 'max':return Math.max(...ns);case 'id':return ns[0];case 'lt':return ns[0]!<ns[1]!;case 'gt':return ns[0]!>ns[1]!;case '=':return ns[0]===ns[1];default:throw Error();}
 }
 try{const ast=parse();if(cursor!==tokens.length)return null;const value=calc(ast);return typeof value==='number'&&Number.isFinite(value)&&Math.abs(value)<=1e15?value:typeof value==='string'?value:null;}catch{return null;}
}
export function treeNodes(kind:'mining'|'foraging',presets:[string,unknown][],tier:number|null):JsonObject[] {
 const refs=object(at(definitions,'trees',kind,'perks'));
 return presets.flatMap(([preset,raw])=>{
  const levels=object(raw),ids=new Set([...Object.keys(refs),...Object.keys(levels).filter(k=>!k.startsWith('toggle_'))]);
  return [...ids].filter(id=>/^[a-z0-9_]{1,64}$/.test(id)).map(id=>{
   const def=object(refs[id]),reported=number(levels[id]),level=reported!==null&&Number.isInteger(reported)&&reported>=0?reported:null,max=number(def.maxLevel),vars={level:Math.max(1,level??0),level0:level??0,maxLevel:max??0,hotm:tier??0};
   const evaluated=Object.fromEntries(Object.entries(def).filter(([k])=>k.startsWith('stat')).map(([k,v])=>[k,level===null?null:ruleNumber(v,vars)]));
   const cost=level!==null&&level>0&&max!==null&&level<max?ruleNumber(def.cost,vars):null;
   const powder=typeof def.powder==='string'&&def.powder.startsWith('(')?ruleNumber(def.powder,vars):text(def.powder);
   return {id,preset,name:text(def.name),level,maximumLevel:max,unlocked:level===null?null:level>0,enabled:typeof levels['toggle_'+id]==='boolean'?levels['toggle_'+id]:null,nextLevelCost:typeof cost==='number'?{amount:Math.floor(cost),currency:powder,status:'calculated_from_pinned_reference'}:null,effects:level===null?null:level===0?{}:evaluated,effectMeaning:level===null?null:{templates:def.lore??null,effectiveGearModifiersIncluded:false},isAbility:typeof def.isAbility==='boolean'?def.isAbility:null,prerequisites:null,layout:{x:def.x??null,y:def.y??null},definitionReasonCode:!Object.keys(def).length?'node_definition_unavailable':level===null?'node_level_not_exposed':cost===null&&max!==null&&level<max?'next_cost_not_present_in_reference':null,rules:{source:definitions.source,revision:definitions.revision,reviewedAt:definitions.reviewedAt,liveGameRulesVerified:false}};
  });
 });
}
