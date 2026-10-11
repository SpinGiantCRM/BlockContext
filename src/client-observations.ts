import { open, constants } from 'node:fs/promises';
import { z } from 'zod';
import { AppError, type JsonObject } from './types.js';

const identifier=z.string().regex(/^[A-Z0-9_:.-]{1,96}$/);
const item=z.object({slot:z.number().int().min(0).max(53),id:identifier,count:z.number().int().min(0).max(127),reforge:z.string().regex(/^[a-z0-9_]{1,64}$/).optional(),enchantments:z.record(z.string().regex(/^[a-z0-9_]{1,64}$/),z.number().int().min(1).max(100)).refine(v=>Object.keys(v).length<=128),hot_potato_count:z.number().int().min(0).max(100).optional(),rarity_upgrades:z.number().int().min(0).max(100).optional(),upgrade_level:z.number().int().min(0).max(100).optional(),dungeon_item_level:z.number().int().min(0).max(100).optional(),art_of_war_count:z.number().int().min(0).max(100).optional(),farming_for_dummies_count:z.number().int().min(0).max(100).optional(),wood_singularity_count:z.number().int().min(0).max(100).optional(),tuned_transmission:z.number().int().min(0).max(100).optional()});
const buff=z.object({active:z.boolean(),remainingSecondsAtObservation:z.number().int().min(0).max(10*365*86400).nullable()});
const observation=z.object({observedAt:z.number().int().nonnegative(),readFrom:z.enum(['tab_display','opened_menu']),values:z.unknown()});
const profile=z.object({uuid:z.string().regex(/^[a-f0-9]{32}$/),playerName:z.string().regex(/^[a-zA-Z0-9_]{1,16}$/),profileName:z.string().regex(/^[A-Za-z][A-Za-z ]{0,31}$/),lastSeenAt:z.number().int().nonnegative().optional(),observations:z.record(z.string().max(96),observation).refine(v=>Object.keys(v).length<=24)});
const snapshot=z.object({schemaVersion:z.literal(1),collectorVersion:z.literal('0.1.0'),exportedAt:z.number().int().nonnegative(),activeProfile:z.string().max(70).nullable(),profiles:z.array(profile).max(8)});
const coins=z.number().finite().min(0).max(1e15);
const bankFields=['personalBankCoins','sharedProfileBankCoins','bankCoinsUnclassified'] as const;
const observedItems=z.object({items:z.array(item).max(54),observedSlots:z.number().int().min(0).max(54),completeStorage:z.literal(false),greenhouseNextStageSecondsAtObservation:z.number().int().min(0).max(10*365*86400).optional()});
const sections=['stats','currencies','equipment','minions','garden'] as const;
export type ClientSection=typeof sections[number];

export class ClientObservations {
  constructor(private readonly path?: string,private readonly now=Date.now) {}
  private async read() {
    if(!this.path)return null;
    let file;
    try {
      file=await open(this.path,constants.O_RDONLY|(constants.O_NOFOLLOW??0));const stat=await file.stat();
      if(!stat.isFile()||stat.size>512*1024||process.platform!=='win32'&&(stat.mode&0o077)!==0)return null;
      return snapshot.parse(JSON.parse(await file.readFile('utf8')));
    }catch{return null;}finally{await file?.close();}
  }
  async query(player:string,requestedProfile?:string,section?:ClientSection):Promise<JsonObject> {
    if(!/^(?:[A-Za-z0-9_]{1,16}|[a-fA-F0-9-]{32,36})$/.test(player))throw new AppError('INVALID_PLAYER','Provide a Minecraft username or UUID.');
    if(requestedProfile!==undefined&&!/^[A-Za-z][A-Za-z ]{0,31}$/.test(requestedProfile))throw new AppError('INVALID_PROFILE','Provide the exact profile name visible in the game.');
    if(section!==undefined&&!sections.includes(section))throw new AppError('INVALID_SECTION','Choose stats, currencies, equipment, minions or garden.');
    const unavailable=(reasonCode:string):JsonObject=>({source:'client_observation',status:'unavailable',reasonCode,observations:{},warnings:['Client observations are distinct from Hypixel API data. Missing observations do not mean zero or empty storage.']});
    const raw=await this.read();if(!raw)return unavailable(this.path?'missing_or_invalid_private_snapshot':'collector_not_configured');
    const now=this.now();if(raw.exportedAt>now+5000||now-raw.exportedAt>86400_000)return unavailable('snapshot_expired_or_future');
    const uuid=player.toLowerCase().replaceAll('-','');
    let matches=raw.profiles.filter(p=>p.uuid===uuid||p.playerName.toLowerCase()===player.toLowerCase());
    if(requestedProfile)matches=matches.filter(p=>p.profileName.toLowerCase()===requestedProfile.toLowerCase());
    else if(matches.length>1)matches=matches.filter(p=>`${p.uuid}:${p.profileName.toLowerCase()}`===raw.activeProfile);
    if(matches.length>1)throw new AppError('PROFILE_REQUIRED','Choose an exact profile name from your collected game data.');
    if(matches.length!==1)return unavailable('no_matching_player_and_profile');
    const p=matches[0]!,out:JsonObject={};
    for(const [key,record] of Object.entries(p.observations)) {
      const age=now-record.observedAt;if(age<0||age>=86400_000)continue;
      let group:ClientSection|undefined,parsed:unknown;
      if(key==='cookie'||key==='godPotion'){group='stats';const result=buff.safeParse(record.values);if(result.success)parsed=result.data;}
      else if(bankFields.some(f=>key===`bank:${f}`)){group='currencies';const field=key.slice(5) as typeof bankFields[number];const result=z.object({[field]:coins}).safeParse(record.values);if(result.success)parsed=result.data;}
      else if(key==='huntaxe'){group='equipment';const result=z.object({weapon:item}).safeParse(record.values);if(result.success)parsed=result.data;}
      else if(key==='toolkit'||/^storage:(?:(?:Small |Medium |Large |Greater |Jumbo )?Backpack(?: \(Slot #\d{1,2}\))?|Ender Chest(?: \(\d+\/\d+\))?|Storage \(\d+\/\d+\))$/.test(key)){group='equipment';const result=observedItems.safeParse(record.values);if(result.success)parsed=result.data;}
      else if(key==='minion'||key==='garden'){group=key==='minion'?'minions':'garden';const result=observedItems.safeParse(record.values);if(result.success)parsed=result.data;}
      if(!parsed||section&&section!==group)continue;
      out[key]={source:'client_observation',status:age>300_000?'stale':'available',observedAt:new Date(record.observedAt).toISOString(),ageSeconds:Math.floor(age/1000),readFrom:record.readFrom,values:parsed as JsonObject,coverage:key==='huntaxe'||key.startsWith('bank:')||group==='stats'?'only_the_displayed_field':'partial_only_viewed_menu',currentStateVerified:false};
    }
    return {source:'client_observation',status:Object.keys(out).length?'partial':'unavailable',reasonCode:Object.keys(out).length?'only_manually_viewed_data':'no_valid_recent_observations',player:{uuid:p.uuid,name:p.playerName},profile:{name:p.profileName},collectorVersion:raw.collectorVersion,exportedAt:new Date(raw.exportedAt).toISOString(),connectedAtLastExport:raw.activeProfile===`${p.uuid}:${p.profileName.toLowerCase()}`&&now-raw.exportedAt<45000,observations:out,warnings:['Owner-only client observations; they are not Hypixel API responses or independently verified values.','Timers are remaining time when observed, not a live countdown. Gameplay can pause effects.','Only manually viewed menus are covered. Unviewed storage and minions remain unknown. Data expires after 24 hours; each key retains only its latest observation.']};
  }
}
