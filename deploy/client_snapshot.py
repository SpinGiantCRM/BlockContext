"""Validate and project latest owner-only collector data; no histories or credentials."""
import json, math, re, time
LIMIT = 512 * 1024
UPGRADES = ('hot_potato_count', 'rarity_upgrades', 'upgrade_level', 'dungeon_item_level', 'art_of_war_count', 'farming_for_dummies_count', 'wood_singularity_count', 'tuned_transmission')

def integer(value, low, high):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value != int(value) or not low <= value <= high:
        raise ValueError('invalid integer')
    return int(value)

def text(value, pattern):
    if not isinstance(value, str) or not re.fullmatch(pattern, value):
        raise ValueError('invalid identifier')
    return value

def item(value):
    out = dict(slot=integer(value['slot'], 0, 53), id=text(value['id'], r'[A-Z0-9_:.-]{1,96}'), count=integer(value['count'], 0, 127))
    if 'reforge' in value: out['reforge'] = text(value['reforge'], r'[a-z0-9_]{1,64}')
    enchants = value.get('enchantments', {})
    if not isinstance(enchants, dict) or len(enchants) > 128: raise ValueError('invalid enchantments')
    out['enchantments'] = {text(k, r'[a-z0-9_]{1,64}'): integer(v, 1, 100) for k, v in enchants.items()}
    for k in UPGRADES:
        if k in value: out[k] = integer(value[k], 0, 100)
    for k in ('art_of_peace_count','etherwarp','mana_disintegrator_count','drill_fuel','drill_fuel_capacity','fuel','pickonimbus_durability','cultivating','farmed_cultivating','compact_blocks','expertise_kills'):
        if k in value:
            n=value[k]
            if isinstance(n,bool) or not isinstance(n,(int,float)) or not math.isfinite(n) or not 0<=n<=1e15:raise ValueError('invalid upgrade')
            out[k]=n
    if 'abilityScrolls' in value:
        if not isinstance(value['abilityScrolls'],list) or len(value['abilityScrolls'])>16:raise ValueError('invalid scrolls')
        out['abilityScrolls']=[text(v,r'[A-Z_]{1,96}') for v in value['abilityScrolls']]
    for k in ('attributes','runes'):
        if k in value:
            if not isinstance(value[k],dict) or len(value[k])>64:raise ValueError('invalid attributes')
            out[k]={text(key,r'[a-zA-Z0-9_:.-]{1,96}'):integer(v,0,100) for key,v in value[k].items()}
    for k in ('drill_part_engine','drill_part_fuel_tank','drill_part_upgrade_module','skin'):
        if k in value:out[k]=text(value[k],r'[a-zA-Z0-9_:.-]{1,96}')
    if 'gems' in value:
        if not isinstance(value['gems'],dict) or len(value['gems'])>64:raise ValueError('invalid gems')
        gems={}
        for k,v in value['gems'].items():
            text(k,r'[A-Z0-9_]{1,64}')
            if isinstance(v,str):gems[k]=text(v,r'[A-Z0-9_]{1,32}')
            elif isinstance(v,dict):gems[k]={key:text(v[key],r'[A-Z0-9_]{1,32}') for key in ('quality','type') if key in v}
            else:raise ValueError('invalid gem')
        out['gems']=gems
    if 'instanceId'  in value:out['instanceId']=text(value['instanceId'],r'[a-f0-9]{64}')
    if 'displayName' in value:out['displayName']=text(value['displayName'],r"[A-Za-z0-9 '():+./%✪★-]{1,96}")
    if 'rarity' in value:out['rarity']=text(value['rarity'],r'COMMON|UNCOMMON|RARE|EPIC|LEGENDARY|MYTHIC|DIVINE|SPECIAL|VERY SPECIAL')
    if 'itemType' in value:out['itemType']=text(value['itemType'],r'[A-Z ]{1,32}')
    if 'storedWeapon' in value:
        v=value['storedWeapon']
        if v['status']!='partial' or v['completeWeaponDetails'] is not False:raise ValueError('invalid stored summary')
        out['storedWeapon']=dict(status='partial',displayName=text(v['displayName'],r"[A-Za-z][A-Za-z '-]{0,95}"),stars=None if v['stars'] is None else integer(v['stars'],1,15),completeWeaponDetails=False)
    return out

LABELS=set('Level|Tier|Experience|HotM Experience|HotF Experience|Mithril Powder|Gemstone Powder|Glacite Powder|Tokens of the Mountain|Tokens of the Forest|Mining Speed|Mining Fortune|Foraging Fortune|Hunting Fortune|Charm Chance|Pristine|Breaking Power|Magical Power|Bonus Commissions Remaining|Daily Bonus Commissions|Charges|Remaining Charges|Copper|Bits|Motes|Whispers|Chocolate|Progress|Cost|Next Level Cost|Powder Cost|Fuel|Organic Matter|Item Tier|Collection|Health|Defense|Strength|Crit Damage|Crit Chance|Intelligence|Speed|Sea Creature Chance|Fishing Speed'.split('|'))
def facts(value):
    if value['completeSystem'] is not False or not isinstance(value['facts'],list) or len(value['facts'])>54:raise ValueError('invalid facts')
    out=[]
    for row in value['facts']:
        if not isinstance(row['fields'],list) or len(row['fields'])>64:raise ValueError('invalid fields')
        fields=[]
        for f in row['fields']:
            v=f['value'];mx=f['maximum'];unit=f['unit']
            if f['label'] not in LABELS or unit not in (None,'XP','coins','Powder','Whispers','Bits','Motes','%'):raise ValueError('invalid field label')
            if isinstance(v,bool) or not isinstance(v,(int,float)) or not math.isfinite(v) or abs(v)>1e15:raise ValueError('invalid field value')
            if mx is not None and (isinstance(mx,bool) or not isinstance(mx,(int,float)) or not math.isfinite(mx) or not 0<=mx<=1e15):raise ValueError('invalid maximum')
            fields.append(dict(label=f['label'],value=v,maximum=mx,unit=unit))
        out.append(dict(slot=integer(row['slot'],0,53),displayName=text(row['displayName'],r"[A-Za-z0-9 '():+./%✪★-]{1,96}"),fields=fields))
    return dict(facts=out,observedSlots=integer(value['observedSlots'],0,54),completeSystem=False)

def values(key, value):
    if key in ('cookie', 'godPotion'):
        if not isinstance(value['active'], bool): raise ValueError('invalid buff')
        n = value.get('remainingSecondsAtObservation')
        return dict(active=value['active'], remainingSecondsAtObservation=None if n is None else integer(n, 0, 10*365*86400))
    if key in ('bank:personalBankCoins', 'bank:sharedProfileBankCoins', 'bank:bankCoinsUnclassified'):
        field = key[5:]; n = value[field]
        if isinstance(n, bool) or not isinstance(n, (float, int)) or not math.isfinite(n) or not 0 <= n <= 1e15: raise ValueError('invalid balance')
        return {field: n}
    if key == 'huntaxe': return {'weapon': item(value['weapon'])}
    storage = r'storage:(?:(?:Small |Medium |Large |Greater |Jumbo )?Backpack(?: \(Slot #\d{1,2}\))?|Ender Chest(?: \(\d+/\d+\))?|Storage \(\d+/\d+\))'
    if key in ('inventory','toolkit', 'minion', 'garden') or re.fullmatch(storage, key):
        if value['completeStorage'] is not False or not isinstance(value['items'], list) or len(value['items']) > 54: raise ValueError('invalid menu')
        out=dict(items=[item(i) for i in value['items']], observedSlots=integer(value['observedSlots'], 0, 54), completeStorage=False)
        if key=='garden' and 'greenhouseNextStageSecondsAtObservation' in value: out['greenhouseNextStageSecondsAtObservation']=integer(value['greenhouseNextStageSecondsAtObservation'],0,10*365*86400)
        return out
    if key in ('tabStats','mining','foraging','hunting','dailies','stats','pets','collections','accessories','quests'):return facts(value)
    return None

def project(blob, owner, now=None):
    if len(blob) > LIMIT: raise ValueError('oversized snapshot')
    owner = text(owner, r'[a-f0-9]{32}')
    root = json.loads(blob)
    if root['schemaVersion'] != 1 or root['collectorVersion'] not in ('0.1.0', '0.1.1', '0.2.0', '0.2.1'): raise ValueError('unsupported schema')
    now = int(time.time()*1000) if now is None else now
    exported = integer(root['exportedAt'], now-86400_000, now+5000)
    profiles = root['profiles']
    if not isinstance(profiles, list) or not 1 <= len(profiles) <= 8: raise ValueError('invalid profiles')
    result=[]; keys=set()
    for p in profiles:
        if p['uuid'] != owner: raise ValueError('other player data rejected')
        name=text(p['playerName'], r'[A-Za-z0-9_]{1,16}'); profile=text(p['profileName'], r'[A-Za-z][A-Za-z ]{0,31}')
        key=owner+':'+profile.lower()
        if key in keys: raise ValueError('duplicate profile')
        keys.add(key)
        source=p['observations']
        if not isinstance(source, dict) or len(source)>24: raise ValueError('invalid observations')
        obs={}
        for k, record in source.items():
            if not isinstance(k,str) or len(k)>96: raise ValueError('invalid field')
            v=values(k,record['values'])
            if v is None: continue
            observed=integer(record['observedAt'], 0, now+5000)
            if observed < now-86400_000: continue
            if record['readFrom'] not in ('opened_menu','tab_display','player_inventory'): raise ValueError('invalid source')
            obs[k]=dict(observedAt=observed, readFrom=record['readFrom'], values=v)
        result.append(dict(uuid=owner, playerName=name, profileName=profile, observations=obs))
    active=root.get('activeProfile')
    if active is not None and active not in keys: raise ValueError('invalid active profile')
    out=dict(schemaVersion=1,collectorVersion=root['collectorVersion'],exportedAt=exported,activeProfile=active,profiles=result)
    return json.dumps(out, separators=(',',':'), allow_nan=False).encode()
