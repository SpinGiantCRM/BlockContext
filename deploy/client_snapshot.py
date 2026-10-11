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
    return out

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
    if key in ('toolkit', 'minion', 'garden') or re.fullmatch(storage, key):
        if value['completeStorage'] is not False or not isinstance(value['items'], list) or len(value['items']) > 54: raise ValueError('invalid menu')
        out=dict(items=[item(i) for i in value['items']], observedSlots=integer(value['observedSlots'], 0, 54), completeStorage=False)
        if key=='garden' and 'greenhouseNextStageSecondsAtObservation' in value: out['greenhouseNextStageSecondsAtObservation']=integer(value['greenhouseNextStageSecondsAtObservation'],0,10*365*86400)
        return out
    return None

def project(blob, owner, now=None):
    if len(blob) > LIMIT: raise ValueError('oversized snapshot')
    owner = text(owner, r'[a-f0-9]{32}')
    root = json.loads(blob)
    if root['schemaVersion'] != 1 or root['collectorVersion'] not in ('0.1.0', '0.1.1'): raise ValueError('unsupported schema')
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
            if record['readFrom'] not in ('opened_menu','tab_display'): raise ValueError('invalid source')
            obs[k]=dict(observedAt=observed, readFrom=record['readFrom'], values=v)
        result.append(dict(uuid=owner, playerName=name, profileName=profile, observations=obs))
    active=root.get('activeProfile')
    if active is not None and active not in keys: raise ValueError('invalid active profile')
    out=dict(schemaVersion=1,collectorVersion=root['collectorVersion'],exportedAt=exported,activeProfile=active,profiles=result)
    return json.dumps(out, separators=(',',':'), allow_nan=False).encode()
