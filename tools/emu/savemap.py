"""savemap.py : turn the load log of runfull.py into a save map and a coverage report.

    savemap.py [FULLMAP] [SAVE]    -> data/save-map.csv, data/save-coverage.txt

Every byte or bit read of the game's loader becomes a row. Repeated reads (same loader
pc sequence at a constant stride) collapse into one array row. Rows are labelled from
LABELS (offsets relative to the character base or absolute, see docs/11-save-map.md).
"""
import os, sys, pickle, csv, collections

ROOT = os.path.dirname(os.path.abspath(__file__)) + '/../../'
FULLMAP = sys.argv[1] if len(sys.argv) > 1 else ROOT + 'scratch/fullmap.pkl'
SAVE = sys.argv[2] if len(sys.argv) > 2 else ROOT + 'scratch/work/system.ro'
OBJ, OBJ_SZ = 0x40000000, 0x1000000

# loader function -> manager. Singleton names are the MT class (DTI) name referenced in the
# same translation unit as the loader; '?' = nearest name, not certain.
MANAGERS = {
    0x52163c: 'sUserInfo (common)', 0x263298: 'sOtomo (common)', 0x2257e4: 'sBlackList', 0x16424c: 'sGuildCard (common)',
    0x3f8ef8: 'sGameControl (common)', 0x36bdc: 'sPrivilege',
    0x3e0db8: 'character header',
    0x19a4ec: 'sItemBox', 0x19e0ac: 'sItemPouch', 0x51dbb4: 'sUserInfo', 0x14b0b0: 'sEquipBox',
    0x3f8de0: 'sGameControl', 0x1979b0: 'sItem? (activity)', 0x2755f0: 'sPlayer', 0x2639ac: 'sOtomo',
    0x507d44: 'sVillage', 0x240d60: 'sNpcTalk', 0x1a4e64: 'sKitchen', 0x3f43d4: 'sFlagChecker',
    0x14c568: 'sEventCtrl?', 0x163e0c: 'sGuildCard', 0x15bd3c: 'sGuestHunter', 0x539120: 'sTutorial',
    0x1c9388: 'sMonNyan', 0x55d2c0: 'chat phrases (0x55d2c0)', 0x521398: 'sUserInfo fixup', 0x2282a8: 'tail 0x2282a8',
}

# (scope, offset, size, label, confidence); scope 'char' = relative to the character base
LABELS = [
    ('abs', 0x0, 0x24, 'Switch header (nonce at 0x14)', 'CONFIRMED'),
    ('abs', 0x24, 0x1c, 'body header: version 0xc6, 1, block A/B offsets, 3 character offsets', 'CONFIRMED'),
    ('char', 0x0, 0x278, 'character header (name +0, HR +0x28, art slots +0x2C, equipped cache +0x110, pigment +0x24C)', 'DERIVED'),
    ('char', 0x278, 0x1557, 'item box: 2300 x (u12 item ID, u7 count), bit stream', 'CONFIRMED'),
    ('char', 0x17CF, 0xFF0, 'item loadouts: 24 x 170 B (name char[42], 32 x (u16 item, u16 count))', 'DERIVED'),
    ('char', 0x27BF, 0x4C, 'item pouch: 32 x (u12 item ID, u7 count), bit stream', 'CONFIRMED'),
    ('char', 0x280B, 0x400, 'S+0x20 talk state block (contribution points +0x10/+0x20)', 'DERIVED'),
    ('char', 0x281B, 0x10, 'Village contribution points, low rank', 'DERIVED'),
    ('char', 0x282B, 0x10, 'Village contribution points, G rank', 'DERIVED'),
    ('char', 0x283C, 0x12, 'deviant permit counts', 'CONFIRMED'),
    ('char', 0x2C13, 0x18, 'Hunter Arts unlocked', 'CONFIRMED'),
    ('char', 0x2C63, 0x14, 'Palico learned maps', 'DERIVED'),
    ('char', 0x2C77, 0x100, 'quests cleared bitmap', 'CONFIRMED'),
    ('char', 0x2D77, 0x100, 'quests seen bitmap', 'CONFIRMED'),
    ('char', 0x2E77, 0x100, 'quests failed bitmap', 'DERIVED'),
    ('char', 0x2F77, 0x4, 'progress word', 'DERIVED'),
    ('char', 0x2F8F, 0x6, 'Canteen ingredients', 'CONFIRMED'),
    ('char', 0x3157, 0x14, 'awards, game-side map', 'DERIVED'),
    ('char', 0x316B, 0x14, 'award notices', 'DERIVED'),
    ('char', 0x3187, 0x10, 'quest sets completed', 'CONFIRMED'),
    ('char', 0x3197, 0x10, 'quest set notices', 'DERIVED'),
    # S fields named from the code (docs/11-save-map.md#the-save-object-s). "NEW" = the saved notice copy of
    # an unlock map: the game sets it with each newly set bit and clears it when the item is shown.
    ('char', 0x2C0B, 0x4, 'bonus packs announced: bit N = privilege pack N notice shown (S+0x424)', 'DERIVED'),
    ('char', 0x2C0F, 0x4, 'bonus packs granted: bit N = contents of privilege pack N given (S+0x428)', 'DERIVED'),
    ('char', 0x2C2B, 0x18, 'Hunter Arts unlocked, runtime NEW copy (S+0xd3c)', 'DERIVED'),
    ('char', 0x2C43, 0x18, 'Hunter Arts unlocked, NEW (S+0xd54)', 'DERIVED'),
    ('char', 0x2C5B, 0x4, '31-bit map committed from S+0xd74 at quest end, counted by Alchemy (S+0xd6c)', 'DERIVED'),
    ('char', 0x2C5F, 0x8, 'u32 map (S+0xd70) + pending 31-bit map set by monster code (S+0xd74)', 'DERIVED'),
    ('char', 0x2F7F, 0x8, 'progress map NEW (S+0x968)', 'DERIVED'),
    ('char', 0x2F87, 0x8, 'unlock map set by the quest flow, bits 2-8 (S+0x970) + runtime NEW copy', 'UNRESOLVED'),
    ('char', 0x2F8B, 0x4, 'unlock map NEW (S+0x978)', 'UNRESOLVED'),
    ('char', 0x2F97, 0x8, 'Canteen ingredients NEW (S+0x98c)', 'DERIVED'),
    ('char', 0x2F9F, 0x8, 'Poogie costumes owned, 64 bits (S+0x994)', 'DERIVED'),
    ('char', 0x2FA7, 0x8, 'Poogie costumes NEW (S+0x9a4)', 'DERIVED'),
    ('char', 0x2FAF, 0x8, "deviants: bit i = deviant i's first Special Permit quest unlocked (S+0x9ac) + runtime NEW copy", 'DERIVED'),
    ('char', 0x2FB3, 0x4, 'deviants unlocked, NEW (S+0x9b4)', 'DERIVED'),
    ('char', 0x2FB7, 0x8, 'deviants offered by the Courier, 18 bits (S+0x9b8) + runtime NEW copy', 'DERIVED'),
    ('char', 0x2FBB, 0x4, 'deviants offered by the Courier, NEW (S+0x9c0)', 'DERIVED'),
    ('char', 0x2FBF, 0xC, 'unlock map S+0x9c4 with NEW copies, no reader found', 'UNRESOLVED'),
    ('char', 0x2FC7, 0xA4, 'Guild Card title words, part 1 unlocked: 1312 bits, 1309 words (GC_Title_1)', 'DERIVED'),
    ('char', 0x306B, 0xA4, 'Guild Card title words, part 1 NEW', 'DERIVED'),
    ('char', 0x310F, 0x10, 'Guild Card title words, part 2 unlocked: 121 words (GC_Title_2)', 'DERIVED'),
    ('char', 0x311F, 0x10, 'Guild Card title words, part 2 NEW', 'DERIVED'),
    ('char', 0x312F, 0x14, 'Guild Card scenes unlocked: 136 (GC_background)', 'DERIVED'),
    ('char', 0x3143, 0x14, 'Guild Card scenes NEW', 'DERIVED'),
    ('char', 0x317F, 0x4, 'Guild Card poses unlocked: 22', 'DERIVED'),
    ('char', 0x3183, 0x4, 'Guild Card poses NEW', 'DERIVED'),
    ('char', 0x31A7, 0x24, 'Smithy decorations listed: bit = rDecoCreateData entry', 'DERIVED'),
    ('char', 0x31CB, 0x24, 'Smithy decorations NEW', 'DERIVED'),
    ('char', 0x31EF, 0x8, 'Trader map 1, 32 bits (S+0x3488) + runtime NEW copy', 'DERIVED'),
    ('char', 0x31F3, 0x4, 'Trader map 1 NEW', 'DERIVED'),
    ('char', 0x31F7, 0x8, 'Trader map 2, 32 bits (S+0x3494) + runtime NEW copy', 'DERIVED'),
    ('char', 0x31FB, 0x4, 'Trader map 2 NEW', 'DERIVED'),
    ('char', 0x31FF, 0x38, 'Trader limited trades, 448 bits (tradeLimitedHonorList has 442)', 'DERIVED'),
    ('char', 0x3237, 0x38, 'Trader limited trades NEW', 'DERIVED'),
    ('char', 0x326F, 0x14, 'Trader limited trades, 160 bits (tradeLimitedPaperList has 131)', 'DERIVED'),
    ('char', 0x3283, 0x14, 'Trader limited trades, 160 bits, NEW', 'DERIVED'),
    ('char', 0x3297, 0x4, 'Trader map, 32 bits (S+0x3584)', 'DERIVED'),
    ('char', 0x329B, 0x4, 'Trader map S+0x3584 NEW', 'DERIVED'),
    ('char', 0x329F, 0x4, 'Trader ticket map, 32 bits (S+0x3590)', 'DERIVED'),
    ('char', 0x32A3, 0x4, 'Trader ticket map NEW (also read by the Cross ticket screen)', 'DERIVED'),
    ('char', 0x32A7, 0x4, 'Trader map, 32 bits (S+0x359c)', 'DERIVED'),
    ('char', 0x32AB, 0x4, 'Trader map S+0x359c NEW', 'DERIVED'),
    ('char', 0x32AF, 0x4, 'u32 flags read by the Trader and the Start Menu (S+0x35a8)', 'UNRESOLVED'),
    ('char', 0x32B3, 0x4, "Hunter's Notes tips read: clear bit = NEW (S+0x35ac)", 'DERIVED'),
    ('char', 0x32B7, 0x10, "Hunter's Notes, large monsters: 123 bits (S+0x35b0)", 'DERIVED'),
    ('char', 0x32C7, 0x10, "Hunter's Notes, large monsters NEW (S+0x35c0)", 'DERIVED'),
    ('char', 0x32D7, 0x4, "Hunter's Notes, second list: 30 bits (S+0x35d0)", 'DERIVED'),
    ('char', 0x32DB, 0x8, 'two u32 maps compared with the DLC map sPrivilege+0xf3c by the Room Service (S+0x35dc)', 'UNRESOLVED'),
    ('char', 0x32E3, 0x4, 'network flags (sFestaNetwork; runtime part at S+0x366c)', 'UNRESOLVED'),
    ('char', 0x32E7, 0x20, 'shop list 1 listed: 256 bits (uUIGuildShop, S+0xd98)', 'DERIVED'),
    ('char', 0x3307, 0x20, 'shop list 1 NEW (S+0xdd8)', 'DERIVED'),
    ('char', 0x3327, 0x20, 'shop list 2 listed: 256 bits (S+0xdf8)', 'DERIVED'),
    ('char', 0x3347, 0x20, 'shop list 2 NEW (S+0xe38)', 'DERIVED'),
    ('char', 0x3367, 0x348, 'Armory (equipment shop): 21 types x (20 B listed, 20 B NEW), bit = shop entry', 'DERIVED'),
    ('char', 0x36AF, 0x258, 'Smithy weapon lists, types 7-21: 15 x (20 B listed, 20 B NEW)', 'DERIVED'),
    ('char', 0x3907, 0x120, 'Smithy armor list, head: listed', 'DERIVED'),
    ('char', 0x3A27, 0x120, 'Smithy armor list, head: NEW', 'DERIVED'),
    ('char', 0x3B47, 0x120, 'Smithy armor list, chest: listed', 'DERIVED'),
    ('char', 0x3C67, 0x120, 'Smithy armor list, chest: NEW', 'DERIVED'),
    ('char', 0x3D87, 0x120, 'Smithy armor list, arms: listed', 'DERIVED'),
    ('char', 0x3EA7, 0x120, 'Smithy armor list, arms: NEW', 'DERIVED'),
    ('char', 0x3FC7, 0x120, 'Smithy armor list, waist: listed', 'DERIVED'),
    ('char', 0x40E7, 0x120, 'Smithy armor list, waist: NEW', 'DERIVED'),
    ('char', 0x4207, 0x120, 'Smithy armor list, legs: listed', 'DERIVED'),
    ('char', 0x4327, 0x120, 'Smithy armor list, legs: NEW', 'DERIVED'),
    ('char', 0x4447, 0x44, 'Palico smithy, weapons: listed', 'DERIVED'),
    ('char', 0x448B, 0x44, 'Palico smithy, weapons: NEW', 'DERIVED'),
    ('char', 0x44CF, 0x44, 'Palico smithy, helms: listed', 'DERIVED'),
    ('char', 0x4513, 0x44, 'Palico smithy, helms: NEW', 'DERIVED'),
    ('char', 0x4557, 0x44, 'Palico smithy, mail: listed', 'DERIVED'),
    ('char', 0x459B, 0x44, 'Palico smithy, mail: NEW', 'DERIVED'),
    ('char', 0x45DF, 0x780, 'Smithy weapon maps by weapon ID: 15 types x 1024 bits (set by the list builder 0x6ffd40)', 'DERIVED'),
    ('char', 0x4D5F, 0x290, 'Smithy armor map by armor ID: 5248 bits (set by the list builder 0x6ffd40)', 'DERIVED'),
    ('char', 0x4FEF, 0xC, 'S+0xd8c: 12 bytes read by NPCs, the Footbath and Alchemy', 'UNRESOLVED'),
    ('char', 0x4FFB, 0x28, 'Arena: bit 5 x quest + set = cleared with that equipment set (S+0xca0)', 'DERIVED'),
    ('char', 0x5023, 0x28, 'Arena equipment sets NEW (S+0xcf0)', 'DERIVED'),
    ('char', 0x504B, 0x8, 'rotating quests bitmap', 'CONFIRMED'),
    ('char', 0x5053, 0x4, 'u32 cleared by the quest result flow 0x38b948 (S+0x3670)', 'UNRESOLVED'),
    ('char', 0x505B, 0x8, 'Jukebox songs unlocked (S+0x35e4) + runtime NEW copy', 'DERIVED'),
    ('char', 0x505F, 0x4, 'Jukebox songs NEW (S+0x35ec)', 'DERIVED'),
    ('char', 0x5063, 0x18, 'Lab upgrades offered, 96 bits (S+0x35f0) + runtime NEW copy', 'DERIVED'),
    ('char', 0x506F, 0xC, 'Lab upgrades offered NEW (S+0x3608)', 'DERIVED'),
    ('char', 0x507B, 0x18, 'Lab upgrades installed, bit = researchReinforce ID - 1; bits 0-2 = Item Box expansions (S+0x3614) + runtime NEW copy', 'DERIVED'),
    ('char', 0x5087, 0xC, 'Lab upgrades installed NEW (S+0x362c)', 'DERIVED'),
    ('char', 0x5093, 0x8, 'supply drop sets unlocked (Provision Division, S+0x3638) + runtime NEW copy', 'DERIVED'),
    ('char', 0x509B, 0x8, 'supply drop sets NEW (S+0x3648)', 'DERIVED'),
    ('char', 0x50A3, 0x8, 'Cross coin trades unlocked (S+0x3650) + runtime NEW copy', 'DERIVED'),
    ('char', 0x50AB, 0x8, 'Cross coin trades NEW (S+0x3660)', 'DERIVED'),
    ('char', 0x50B3, 0x4, "deviants: bit i = deviant i's last Special Permit quest (level 16) cleared (S+0xd18)", 'DERIVED'),
    ('char', 0x50B7, 0x4, 'deviant last level cleared, NEW (S+0xd20)', 'DERIVED'),
    ('char', 0x50BB, 0x3, 'u8 + u16 (S+0x3680, S+0x3682); the u16 is recomputed at load (0x6b1e6c)', 'DERIVED'),
    ('char', 0x50BE, 0xD74, 'S+0x3684: zero in the analysed save, cleared with S+0x3682 at init, no reader found', 'UNRESOLVED'),
    ('char', 0x5057, 0x4, 'u32 of the chat-phrase object (+0x2838) loaded inside the S stream; a value >= 0 is replaced by 0xF8FC7E3F at load', 'DERIVED'),
    ('char', 0x5E32, 0x3, 'S+0x43f8, no reader found', 'UNRESOLVED'),
    ('char', 0x5E35, 0x1, 'control option byte: set by the Game options window, read by the player and the target camera (S+0x43fb)', 'DERIVED'),
    ('char', 0x5E36, 0x4, 'Courier: deviant bit lent for temporary mode 1, to restore (S+0x43fc)', 'DERIVED'),
    ('char', 0x5E3A, 0x4, 'Courier: deviant bit lent for temporary mode 2, to restore (S+0x4400)', 'DERIVED'),
    ('char', 0x5E3E, 0x8, 'u32 pair get/set by the title menu (S+0x4408)', 'UNRESOLVED'),
    ('char', 0x5E46, 0x4, 'u32 (S+0x4410)', 'UNRESOLVED'),
    ('char', 0x5E4A, 0x4, 'Courier flags, u32 (S+0x4414)', 'DERIVED'),
    ('char', 0x5E4E, 0x2, 'quest counter', 'DERIVED'),
    ('char', 0x5E52, 0x4, 'quest counter reference: both folded into [210, 420) by 0x5279e0 (S+0x441c)', 'DERIVED'),
    ('char', 0x5E56, 0x4, 'Courier points: added per quest, turned into Special Permits (S+0x4420)', 'DERIVED'),
    ('char', 0x5E5A, 0x24, 'Special Permit points per deviant, 18 x u16, at most 9999 (100 = 1 permit, S+0x4424)', 'DERIVED'),
    ('char', 0x5E7E, 0x24, 'Special Permit points waiting at the Courier, 18 x u16 (S+0x4448)', 'DERIVED'),
    ('char', 0x5EA2, 0x4, 'control option bytes: target camera, Hunter Art gauge (S+0x446c)', 'DERIVED'),
    ('char', 0x5EA6, 0x112, 'monster hunt tallies, u16 index 1-137', 'CONFIRMED'),
    ('char', 0x5FB8, 0x112, 'monster capture counts, u16 index 1-137', 'CONFIRMED'),
    ('char', 0x60CA, 0x224, 'monster size records, (u16 min, u16 max) index 1-137', 'CONFIRMED'),
    ('char', 0x62EE, 0x11940, 'equipment box: 2000 x 36 B', 'CONFIRMED'),
    ('char', 0x17C2E, 0x8CA0, 'Palico equipment box: 1000 x 36 B (types 22-24)', 'DERIVED'),
    ('char', 0x208CE, 0x1540, 'My Sets: 40 x 136 B (name at +0)', 'CONFIRMED'),
    ('char', 0x21E0E, 0x660, 'Palico equipment sets: 24 x 68 B (name char[42], 3 x u16 box index)', 'DERIVED'),
    ('char', 0x2381E, 0x17, 'activity: pending village rewards', 'DERIVED'),
    ('char', 0x23A58, 0x145, 'player record (loaded copy of the slot header fields)', 'DERIVED'),
    ('char', 0x23B53, 0x24, 'current pigment, 5 x RGBA + 16 B', 'DERIVED'),
    ('char', 0x23B7B, 0x2, 'current pigment default-colour flags', 'DERIVED'),
    ('char', 0x23B7D, 0x20, 'hunter name', 'DERIVED'),
    ('char', 0x23BB6, 0x6A50, 'Palicoes: 84 x 324 B (name char[32] +0, exp u32 +0x20, level u8 +0x24, greeting +0x60, owner +0x9C)', 'DERIVED'),
    ('char', 0x2A606, 0x1E60, 'Palicoes, second list: 24 x 324 B, same record', 'DERIVED'),
    ('char', 0x2C6BD, 0x9AB00, 'Guild Card list 1: 100 elements (u32 len, zlib card, u32 state, 36 B trailer) + padding', 'DERIVED'),
    ('char', 0xC71BD, 0x18B8, 'own Guild Card, 6328 B (name UTF-16 +0, HR +0x16, equipment 7 x 44 +0x54, Palicoes 3 x 580 +0x188, weapon usage +0x8BA, history +0x918, awards +0xF58)', 'DERIVED'),
    ('char', 0xC8A75, 0x1130, 'Guild Card manager +0x18D8, 4400 B', 'UNRESOLVED'),
    ('char', 0xC9BA5, 0x4D580, 'Guild Card list 2: 50 elements + padding', 'DERIVED'),
    ('char', 0x117125, 0x708, 'Guild Card manager +0x2A0C, 1800 B', 'UNRESOLVED'),
    ('char', 0x11782D, 0x114, 'Guild Card manager +0x3114, 276 B', 'UNRESOLVED'),
    ('char', 0x117941, 0x35E8, 'Guild Card manager +0x3228, 13800 B', 'UNRESOLVED'),
    ('char', 0x11AF29, 0x708, 'Guild Card manager +0x6810, 1800 B', 'UNRESOLVED'),
    ('char', 0x11B631, 0x1868, 'guest hunters: hunters met online (UTF-16 name, greeting, hired copies)', 'DERIVED'),
    ('char', 0x11CE99, 0xA0, 'tutorial flags (8 + 152 B)', 'DERIVED'),
    ('char', 0x11CF39, 0x107, 'Meownster Hunters (sMonNyan) state', 'DERIVED'),
    ('char', 0x11D040, 0x2883, 'chat phrases: 104 B slots (auto-chat lines)', 'DERIVED'),
    ('char', 0x2C4DA, 0x2, 'Village star level', 'CONFIRMED'),
    ('char', 0x2C4DC, 0x2, 'Hub star level', 'CONFIRMED'),
    ('char', 0x2C56D, 0xC0, 'event flags (villager requests)', 'CONFIRMED'),
    ('char', 0x2C62D, 0x48, 'per-NPC talk hold bits', 'CONFIRMED'),
    ('char', 0x2C675, 0x4, 'random word (talk conditions)', 'DERIVED'),
    ('char', 0x2C67D, 0xD, 'Canteen dishes', 'CONFIRMED'),
    ('char', 0x2C68D, 0xD, 'Canteen dishes copy', 'UNRESOLVED'),
    ('char', 0xC7A77, 0x1E, 'Guild Card weapon usage, Village', 'CONFIRMED'),
    ('char', 0xC7A95, 0x1E, 'Guild Card weapon usage, Hub', 'CONFIRMED'),
    ('char', 0xC7AB3, 0x1E, 'Guild Card weapon usage, Arena', 'CONFIRMED'),
    ('char', 0xC7AD1, 0x4, 'Guild Card play time (s)', 'DERIVED'),
    ('char', 0xC7AD5, 0x640, 'quest history log, 10 x 160 B', 'DERIVED'),
    ('char', 0xC8115, 0x11, 'Guild Card awards', 'CONFIRMED'),
    ('abs', 0x40, 0x4, 'bonus packs loaded: bit N = privilege pack N (S+0x420, shared)', 'DERIVED'),
    ('abs', 0x44, 0x6, 'shared setting bytes (S+0x3676), zero in the analysed save', 'UNRESOLVED'),
    ('abs', 0x4A, 0x2, 'shared u8 settings (S+0x367c, S+0x367d)', 'UNRESOLVED'),
    ('abs', 0x4C, 0x1, 'TV brightness: scale 0.4 + 0.025 x value (S+0x367e)', 'DERIVED'),
    ('abs', 0x4D, 0x1, 'rumble on/off (S+0x367f)', 'DERIVED'),
    ('abs', 0xB2A2, 0x2, 'shared u8 settings (sGameControl +0x5c, +0x5d)', 'UNRESOLVED'),
    ('abs', 0xB2A4, 0x1, 'text language, 0 = from the system language (sGameControl +0xa5)', 'DERIVED'),
    ('abs', 0xB2A5, 0x8, 'DLC item packs received, 50 bits (sPrivilege +0xf34)', 'DERIVED'),
    ('abs', 0xB2AD, 0x8, 'DLC Palicoes received, 50 bits (sPrivilege +0xf3c)', 'DERIVED'),
    ('abs', 0xB2B5, 0x4, 'DLC extras received, download type 4: 9 entries; bits 0-3 unlock Guild Card poses (sPrivilege +0xf44)', 'DERIVED'),
    ('abs', 0xB2B9, 0xC, 'DLC extras received, download type 5: 80 entries (sPrivilege +0xf48)', 'DERIVED'),
    ('abs', 0xB2C5, 0x4, 'DLC extras received, download type 6: 10 entries (sPrivilege +0xf54)', 'DERIVED'),
    ('abs', 0xB2C9, 0x28, 'DLC extras received, download type 7: 300 entries (sPrivilege +0xf58)', 'DERIVED'),
    ('abs', 0xB2F1, 0x8, 'challenge quests stored, 45 bits = block B slots (sPrivilege +0xf80)', 'DERIVED'),
    ('abs', 0xB2F9, 0x14, 'event quests stored, 160 bits = block B slots (sPrivilege +0xf88)', 'DERIVED'),
    ('abs', 0xB30D, 0x4, 'u32 compared with the download catalog stamp +0xe5c (sPrivilege +0xf9c)', 'DERIVED'),
    ('abs', 0x4E, 0x3F48, 'Palicoes, common pool: 50 x 324 B (DLC Palicoes, owner "Capcom")', 'DERIVED'),
    ('abs', 0x3F96, 0x2580, 'blacklist: 100 x 96 B (64 + 32)', 'DERIVED'),
    ('abs', 0x6516, 0x4D8C, 'sGuildCard common: 3 x 6616 B + 1 B', 'DERIVED'),
    ('abs', 0xB311, 0x1450, 'DLC item pack list: 50 x 104 B', 'DERIVED'),
    ('abs', 0xC761, 0x3138, 'DLC Palico info, 12600 B', 'DERIVED'),
    ('abs', 0xF899, 0x118000, 'event quests: 160 x 0x1C00 (u32 ID, u32 size, ARC)', 'CONFIRMED'),
    ('abs', 0x127899, 0x4EC00, 'challenge quests: 45 x 0x1C00 (u32 ID, u32 size, ARC)', 'CONFIRMED'),
    ('abs', 0x176499, 0x16800, 'challenge quest records: 45 x 0x800 (u32 ID, u32 size, data)', 'DERIVED'),
]


def load(path):
    d = pickle.load(open(path, 'rb'))
    return d, [r for r in d['log'] if r['k'] in ('R', 'RB')]


def key(r): return (r['k'], r['pc'], r['n'], r.get('mgr'))
def pos(r): return r['off'] * 8 + r.get('bit', 0)
def nbits(r): return r['n'] * 8 if r['k'] == 'R' else r['n']


def compress(log):
    """[(first record, records per element, stride in bits, count)]"""
    out, i, N = [], 0, len(log)
    while i < N:
        best = None
        for p in range(1, 9):
            if i + 3 * p > N: break
            ks = [key(r) for r in log[i:i + p]]
            st = pos(log[i + p]) - pos(log[i])
            if st <= 0: continue
            j = i + p
            while j + p <= N and [key(r) for r in log[j:j + p]] == ks and pos(log[j]) - pos(log[j - p]) == st:
                j += p
            cnt = (j - i) // p
            if cnt >= 3 and (best is None or cnt * p > best[2] * best[0]): best = (p, st, cnt)
        if best:
            p, st, cnt = best
            out.append((log[i:i + p], st, cnt)); i += p * cnt
        else:
            out.append(([log[i]], pos(log[i + 1]) - pos(log[i]) if i + 1 < N else nbits(log[i]), 1)); i += 1
    return out


def objfield(r):
    d = r.get('dst')
    if d is None or not (OBJ <= d < OBJ + 160 * OBJ_SZ): return ''
    return 'obj%d+0x%x' % ((d - OBJ) // OBJ_SZ, (d - OBJ) % OBJ_SZ)


def main():
    save = open(SAVE, 'rb').read()
    d, log = load(FULLMAP)
    bases = [0x24 + int.from_bytes(save[0x34 + 4 * i:0x38 + 4 * i], 'little') for i in range(3)]
    a_off = 0x24 + int.from_bytes(save[0x2c:0x30], 'little'); b_off = 0x24 + int.from_bytes(save[0x30:0x34], 'little')

    def block(off):
        for i in (2, 1, 0):
            if off >= bases[i]: return 'char%d' % (i + 1), off - bases[i]
        if off >= b_off: return 'B', off - b_off
        if off >= a_off: return 'A', off - a_off
        return 'header', off

    labs = []
    for scope, o, n, lab, conf in LABELS:
        for b in (bases if scope == 'char' else [0]):
            labs.append((b + o, b + o + n, lab, conf))
    labs.sort()

    def label(lo, hi):
        """smallest label that contains the row, else the labels inside it"""
        hits = [(a, b, l, c) for a, b, l, c in labs if a < hi and lo < b]
        cont = [h for h in hits if h[0] <= lo and hi <= h[1]]
        if cont:
            a, b, l, c = min(cont, key=lambda h: h[1] - h[0])
            return l, c
        inside = [h for h in hits if lo <= h[0]]
        if inside:
            return 'contains: ' + '; '.join(h[2] for h in inside), min((h[3] for h in inside), key=['UNRESOLVED', 'DERIVED', 'CONFIRMED'].index)
        return '', ''

    rows = []
    for recs, st, cnt in compress(log):
        r0 = recs[0]
        start = pos(r0)
        if cnt > 1: end = start + st * cnt
        else: end = start + nbits(r0)
        lo, hi = start // 8, (end + 7) // 8
        blk, rel = block(lo)
        lab, conf = label(lo, hi)
        fields = ' '.join(('%dB' % r['n']) if r['k'] == 'R' else ('%db' % r['n']) for r in recs)
        rows.append({'offset': '0x%X' % lo, 'bit': start % 8, 'size': hi - lo, 'block': blk, 'rel': '0x%X' % rel,
                     'manager': MANAGERS.get(r0.get('mgr'), r0.get('mgr') and hex(r0['mgr'])),
                     'count': cnt, 'stride_bits': st if cnt > 1 else '', 'element': fields,
                     'loader_pc': '0x%x' % r0['pc'], 'object_field': objfield(r0), 'label': lab, 'confidence': conf})
    # slots 2 and 3 are read by the same chain: check they repeat slot 1, then write slot 1 only.
    # The two Guild Card lists hold zlib elements of variable length (fixed total size): not compared.
    var = [(0x2C6BD, 0xC71BD), (0xC9BA5, 0x117125)]
    fixed = lambda r: not any(a <= int(r['rel'], 16) < b for a, b in var)
    shape = lambda c: [(r['rel'], r['bit'], r['size'], r['manager'], r['count'], r['element']) for r in rows if r['block'] == c and fixed(r)]
    same = shape('char1') == shape('char2') == shape('char3')
    print('character slots 2 and 3 repeat the layout of slot 1:', same)
    if same:
        rows = [r for r in rows if r['block'] not in ('char2', 'char3')]
    with open(ROOT + 'data/save-map.csv', 'w', newline='') as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader(); w.writerows(rows)

    cov = d['cov']
    gaps, i = [], 0
    while i < len(save):
        if cov[i]: i += 1; continue
        j = i
        while j < len(save) and not cov[j]: j += 1
        nz = sum(1 for b in save[i:j] if b)
        gaps.append((i, j, nz)); i = j
    with open(ROOT + 'data/save-coverage.txt', 'w') as f:
        read = sum(1 for b in cov if b)
        f.write('bytes read by the loader: %d of %d (%.1f%%)\n' % (read, len(save), 100 * read / len(save)))
        f.write('unread ranges (start, end, size, non-zero bytes in the analysed save, block):\n')
        for a, b, nz in gaps:
            if b - a >= 1:
                f.write('0x%07X 0x%07X %8d %8d %s+0x%X\n' % ((a, b, b - a, nz) + block(a)))
    print(len(rows), 'rows;', len(gaps), 'unread ranges')


if __name__ == '__main__':
    main()
