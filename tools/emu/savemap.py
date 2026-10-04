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
    0x3f8de0: 'sGameControl', 0x1979b0: 'sItem', 0x2755f0: 'sPlayer', 0x2639ac: 'sOtomo',
    0x507d44: 'sVillage', 0x240d60: 'sNpcTalk', 0x1a4e64: 'sKitchen', 0x3f43d4: 'sFlagChecker',
    0x14c568: 'sMakeAmulet', 0x163e0c: 'sGuildCard', 0x15bd3c: 'sGuestHunter', 0x539120: 'sTutorial',
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
    ('char', 0x280B, 0x400, 'S+0x20..0x41f in object order: HR points +0, funds +4, Wycademy points +0xC, contribution points +0x10/+0x20, permits held +0x31, style use counts +0xFA, Arena Latest Updates +0x108/+0x3E4, Arena best times 57 x 12 B +0x114, daily picks +0x3EC, Jukebox song +0x3F8, transferred HR +0x3FA (docs/11 S+0x20 block)', 'DERIVED'),
    ('char', 0x281B, 0x10, 'Village contribution points, low rank', 'DERIVED'),
    ('char', 0x282B, 0x10, 'Village contribution points, G rank', 'DERIVED'),
    ('char', 0x283C, 0x12, 'deviant permit counts', 'CONFIRMED'),
    ('char', 0x2C13, 0x18, 'Hunter Arts unlocked', 'CONFIRMED'),
    ('char', 0x2C63, 0xC, 'Palico skills learned: 96 bits, bit = ot_skl entry (S+0xd78)', 'DERIVED'),
    ('char', 0x2C6F, 0x8, 'Palico support moves learned: 57 bits (S+0xd84)', 'DERIVED'),
    ('char', 0x2C77, 0x100, 'quests cleared bitmap', 'CONFIRMED'),
    ('char', 0x2D77, 0x100, 'quests seen bitmap', 'CONFIRMED'),
    ('char', 0x2E77, 0x100, 'quests failed bitmap', 'DERIVED'),
    ('char', 0x2F77, 0x4, 'progress word', 'DERIVED'),
    ('char', 0x2F8F, 0x6, 'Canteen ingredients (bit = kitchenListMenu ingredient, data/canteen.csv)', 'CONFIRMED'),
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
    ('char', 0x2F87, 0x8, 'flagship monster story events, bits 2-8 set by the quest flow (S+0x970) + runtime NEW copy', 'DERIVED'),
    ('char', 0x2F8B, 0x4, 'flagship monster story events NEW (S+0x978)', 'DERIVED'),
    ('char', 0x2F97, 0x8, 'Canteen ingredients NEW (S+0x98c)', 'DERIVED'),
    ('char', 0x2F9F, 0x8, 'Poogie costumes owned, 64 bits (S+0x994)', 'DERIVED'),
    ('char', 0x2FA7, 0x8, 'Poogie costumes NEW (S+0x9a4)', 'DERIVED'),
    ('char', 0x2FAF, 0x8, "deviants: bit i = deviant i's first Special Permit quest unlocked (S+0x9ac) + runtime NEW copy", 'DERIVED'),
    ('char', 0x2FB3, 0x4, 'deviants unlocked, NEW (S+0x9b4)', 'DERIVED'),
    ('char', 0x2FB7, 0x8, 'deviants offered by the Courier, 18 bits (S+0x9b8) + runtime NEW copy', 'DERIVED'),
    ('char', 0x2FBB, 0x4, 'deviants offered by the Courier, NEW (S+0x9c0)', 'DERIVED'),
    ('char', 0x2FBF, 0xC, 'vestigial map S+0x9c4 with NEW copies: only the transfer converter 0x51e6d8 writes it, no reader', 'DERIVED'),
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
    ('char', 0x31EF, 0x8, 'Trader item list 0, 32 bits (S+0x3488) + runtime NEW copy', 'DERIVED'),
    ('char', 0x31F3, 0x4, 'Trader item list 0 NEW', 'DERIVED'),
    ('char', 0x31F7, 0x8, 'Trader item list 1, 32 bits (S+0x3494) + runtime NEW copy', 'DERIVED'),
    ('char', 0x31FB, 0x4, 'Trader item list 1 NEW', 'DERIVED'),
    ('char', 0x31FF, 0x38, 'Trader: Guild Card title words for sale, 448 bits (tradeLimitedHonorList has 442)', 'DERIVED'),
    ('char', 0x3237, 0x38, 'Trader title words NEW', 'DERIVED'),
    ('char', 0x326F, 0x14, 'Trader: Guild Card scenes for sale, 160 bits (tradeLimitedPaperList has 131)', 'DERIVED'),
    ('char', 0x3283, 0x14, 'Trader scenes NEW', 'DERIVED'),
    ('char', 0x3297, 0x4, 'Trader: pet costumes for sale, 32 bits (S+0x3584)', 'DERIVED'),
    ('char', 0x329B, 0x4, 'Trader pet costumes NEW', 'DERIVED'),
    ('char', 0x329F, 0x4, 'Trader coin-ticket trades, bit = rTradeCoinTicketList entry; set only by the transfer (S+0x3590)', 'DERIVED'),
    ('char', 0x32A3, 0x4, 'Trader coin-ticket trades NEW (read by the Cross ticket screen and the Trader)', 'DERIVED'),
    ('char', 0x32A7, 0x4, 'Trader delivery requests offered, bit = rTradeDeliveryList entry, set once the request flag is raised (S+0x359c)', 'DERIVED'),
    ('char', 0x32AB, 0x4, 'Trader delivery requests offered NEW (S+0x35a4)', 'DERIVED'),
    ('char', 0x32AF, 0x4, 'delivery requests delivered: bit b = kind-1 request b, 1-13 (S+0x35a8; talk condition 41)', 'DERIVED'),
    ('char', 0x32B3, 0x4, "Hunter's Notes tips read: clear bit = NEW (S+0x35ac)", 'DERIVED'),
    ('char', 0x32B7, 0x10, "Hunter's Notes, large monsters: 123 bits (S+0x35b0)", 'DERIVED'),
    ('char', 0x32C7, 0x10, "Hunter's Notes, large monsters NEW (S+0x35c0)", 'DERIVED'),
    ('char', 0x32D7, 0x4, "Hunter's Notes, second list: 30 bits (S+0x35d0)", 'DERIVED'),
    ('char', 0x32DB, 0x8, 'DLC Palicoes taken from the Room Service, bit b = bit b of sPrivilege +0xf3c (S+0x35dc)', 'DERIVED'),
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
    ('char', 0x4FEF, 0xC, 'S+0xd8c: 96-bit milestone map read by award checks (Moofy/Poogie moments, Footbath, Palico biases)', 'DERIVED'),
    ('char', 0x4FFB, 0x28, 'Arena: bit 5 x quest + set = cleared with that equipment set (S+0xca0)', 'DERIVED'),
    ('char', 0x5023, 0x28, 'Arena equipment sets NEW (S+0xcf0)', 'DERIVED'),
    ('char', 0x504B, 0x8, 'rotating quests bitmap', 'CONFIRMED'),
    ('char', 0x5053, 0x4, 'bit 0: Moofah Fleeceball given since the last quest (0x6be478; cleared by the quest result 0x38b948, S+0x3670)', 'DERIVED'),
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
    ('char', 0x50B3, 0x4, "deviants: bit i = deviant i's EX Special Permit quest (level 16) cleared (S+0xd18)", 'DERIVED'),
    ('char', 0x50B7, 0x4, 'deviant last level cleared, NEW (S+0xd20)', 'DERIVED'),
    ('char', 0x50BB, 0x3, 'u8 + u16 (S+0x3680, S+0x3682); the u16 is recomputed at load (0x6b1e6c)', 'DERIVED'),
    ('char', 0x50BE, 0xD74, 'S+0x3684: reserved, only init / save / load touch it; zero in all slots', 'DERIVED'),
    ('char', 0x5057, 0x4, 'u32 of the chat-phrase object (+0x2838) loaded inside the S stream; a value >= 0 is replaced by 0xF8FC7E3F at load', 'DERIVED'),
    ('char', 0x5E32, 0x3, 'S+0x43f8: reserved, only save / load touch it', 'DERIVED'),
    ('char', 0x5E35, 0x1, 'control option byte: set by the Game options window, read by the player and the target camera (S+0x43fb)', 'DERIVED'),
    ('char', 0x5E36, 0x4, 'Courier: deviant bit lent for temporary mode 1, to restore (S+0x43fc)', 'DERIVED'),
    ('char', 0x5E3A, 0x4, 'Courier: deviant bit lent for temporary mode 2, to restore (S+0x4400)', 'DERIVED'),
    ('char', 0x5E3E, 0x8, 'u64 Nintendo Account ID linked to the save-transfer server (S+0x4408; shared copy at file 0x12C2D6)', 'DERIVED'),
    ('char', 0x5E46, 0x4, 'S+0x4410: reserved, only save / load touch it', 'DERIVED'),
    ('char', 0x5E4A, 0x4, 'Courier flags, u32 (S+0x4414)', 'DERIVED'),
    ('char', 0x5E4E, 0x4, 'quest counter, u32: completed quests except tours and Training (S+0x4418, 0x526f70)', 'DERIVED'),
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
    ('char', 0x208CE, 0x1540, 'My Sets: 40 x 136 B (name +0, box indices +0x2A, pigment +0x64, default flags +0x7D, style +0x82, Hunter Arts +0x83)', 'CONFIRMED'),
    ('char', 0x21E0E, 0x660, 'Palico equipment sets: 24 x 68 B (name char[42], 3 x u16 box index)', 'DERIVED'),
    ('char', 0x22497, 0x178, 'items obtained: bit = item ID, 94 x u32 (sItem +0x9c)', 'DERIVED'),
    ('char', 0x2260F, 0x88, 'Trader cargo 1, 136 B (sItem +0xea4)', 'DERIVED'),
    ('char', 0x22697, 0x88, 'Trader cargo 2, 136 B (sItem +0xf2c)', 'DERIVED'),
    ('char', 0x2271F, 0x88, 'Trader cargo 3, 136 B (sItem +0xfb4)', 'DERIVED'),
    ('char', 0x227A7, 0x1068, 'Alchemy requests: 10 x 420 B (u8, u8, u16, 3 x 36 B equipment, 7 x (u32, u32, 36 B equipment))', 'DERIVED'),
    ('char', 0x2380F, 0xA, 'village tier bytes (4) and 6 bytes (sItem +0x8c)', 'DERIVED'),
    ('char', 0x2381E, 0x17, 'activity: pending village rewards', 'DERIVED'),
    ('char', 0x23854, 0x100, 'Combination List order, pouch list: flags byte (bit 1 custom order, bit 0 filter), then itemPreData recipe index per position (sItem +0x214)', 'DERIVED'),
    ('char', 0x23954, 0x100, 'Combination List order, item box list, same form (sItem +0x314)', 'DERIVED'),
    ('char', 0x23A54, 0x4, 'Horns Coins traded in total; award 104 at 2000 (sItem +0x98)', 'DERIVED'),
    ('char', 0x23A58, 0x145, 'player record (loaded copy of the slot header fields)', 'DERIVED'),
    ('char', 0x23A59, 0xE0, 'player 224-byte block (header +0x2C): 3 x u16 equipped Hunter Arts, u16 SP Art bits per art slot; bytes 8-223 unused by a hunter (Palico parameters); first 4 u16 copied to the Guild Card +0x4C', 'DERIVED'),
    ('char', 0x23B39, 0xE, 'equipped gear: 7 x u16 equipment box index (weapon, head, chest, arms, waist, legs, talisman), 0xFFFF = none', 'DERIVED'),
    ('char', 0x23B47, 0xC, 'weapon class (+0; 15 = Prowler) and character creation choices (+4 = gender)', 'DERIVED'),
    ('char', 0x23B77, 0x4, 'u32: five 5-bit colour modes, one per pigment slot (0 RGBA, 2+ preset v-2; header +0x274, Guild Card +0x48)', 'DERIVED'),
    ('char', 0x23B53, 0x24, 'current pigment, 5 x RGBA + 16 B', 'DERIVED'),
    ('char', 0x23B7B, 0x2, 'current pigment default-colour flags', 'DERIVED'),
    ('char', 0x23B7D, 0x20, 'hunter name', 'DERIVED'),
    ('char', 0x23BB6, 0x6A50, 'Palicoes: 84 x 324 B (name char[32] +0, exp u32 +0x20, level u8 +0x24, greeting +0x60, owner +0x9C)', 'DERIVED'),
    ('char', 0x2A606, 0x1E60, 'Palicoes, second list: 24 x 324 B, same record', 'DERIVED'),
    ('char', 0x2C6BD, 0x9AB00, 'Guild Card list 1 (stored cards): 100 elements (u32 len, zlib card, u32 state, 36 B trailer) + padding', 'DERIVED'),
    ('char', 0xC71BD, 0x18B8, 'own Guild Card, 6328 B (name UTF-16 +0, HR +0x16, equipment 7 x 44 +0x54, Palicoes 3 x 580 +0x188, title +0x854, scene +0x85A, pose +0x85B, weapon usage +0x8BA, history +0x918, awards +0xF58, monster log +0xF6C, Arena log +0x1224)', 'DERIVED'),
    ('char', 0xC8A75, 0x1130, 'list 1 card info: 100 x 44 B (date received +0, comment +4, Unity +0x1C, card type +0x20, sender ID +0x21)', 'DERIVED'),
    ('char', 0xC9BA5, 0x4D580, 'Guild Card inbox (list 2): 50 elements + padding', 'DERIVED'),
    ('char', 0x117125, 0x708, 'Guild Card inbox info: 50 x 36 B (date received +0)', 'DERIVED'),
    ('char', 0x11782D, 0x114, 'StreetPass Palico to send: one 276 B record (sender ID +0, name UTF-16 +8, appearance +0x1E, 9 colours +0x2C, parameters +0x50)', 'DERIVED'),
    ('char', 0x117941, 0x35E8, 'Palico inbox: 50 x 276 B, same record', 'DERIVED'),
    ('char', 0x11AF29, 0x708, 'Palico inbox info: 50 x 36 B', 'DERIVED'),
    ('char', 0x11B631, 0x1868, 'guest hunters: hunters met online (UTF-16 name, greeting, hired copies)', 'DERIVED'),
    ('char', 0x11CE99, 0xA0, 'tutorial flags (8 + 152 B)', 'DERIVED'),
    ('char', 0x11CF39, 0x107, 'Meownster Hunters (sMonNyan) state', 'DERIVED'),
    ('char', 0x11D040, 0x2883, 'chat phrases: 104 B slots (auto-chat lines)', 'DERIVED'),
    ('char', 0x2C4DA, 0x2, 'Village star level', 'CONFIRMED'),
    ('char', 0x2C4DC, 0x2, 'Hub star level', 'CONFIRMED'),
    ('char', 0x2C56D, 0xC0, 'event flags (villager requests)', 'CONFIRMED'),
    ('char', 0x2C62D, 0x48, 'per-NPC talk hold bits', 'CONFIRMED'),
    ('char', 0x2C675, 0x4, 'random word (talk conditions)', 'DERIVED'),
    ('char', 0x2C679, 0x4, 'second random word, copied with the first (0x240e4c) and used the same way (0x24217c)', 'DERIVED'),
    ('char', 0x2246E, 0x1D, 'game options, 29 bytes (Game / Chat / Network option windows, quest camera; sGameControl +0x5e)', 'DERIVED'),
    ('char', 0x2248B, 0x4, 'play time in seconds (sGameControl +0x34; slot header +0x20, Guild Card +0x914)', 'DERIVED'),
    ('char', 0x2248F, 0x4, 'f32 play-time remainder in frames, carried into the seconds at 60 (sGameControl +0x38, 0x3f83a8)', 'DERIVED'),
    ('char', 0x22493, 0x4, 'u32 copied to the Guild Card +0x86C (sGameControl +0x3c)', 'DERIVED'),
    ('char', 0x23B9D, 0x5, 'Palico manager: Prowler Palico, buddy 1, buddy 2 (index, 0xFF = none), Dojo sessions done, Palicoes hired (sOtomo +0x13848)', 'DERIVED'),
    ('char', 0x23BA2, 0x14, 'Palico service settings (uUIOtomoService): u2 mode, seven small levels (max 9, 6, then 10 each), four u32 (sOtomo +0x138f6)', 'DERIVED'),
    ('char', 0x2C466, 0x28, 'Palicoes that reached level 50: 5 x 8 B ID; award 50 (sOtomo +0x13910)', 'DERIVED'),
    ('char', 0x2C48E, 0x10, 'Palico Dojo teaching session: 4 indices, kind, skill/move, u16 in progress, two u32 (sOtomo +0x48a78)', 'DERIVED'),
    ('char', 0x2C49E, 0x30, 'Palico Dojo training slots: 3 x 16 B (index, type, sessions left; sOtomo +0x48a88)', 'DERIVED'),
    ('char', 0x2C4CE, 0x6, 'Palico team of up to 5 (Meownster Hunters?): u8 count, 5 x u8 index (sOtomo +0x48ab8)', 'DERIVED'),
    ('char', 0x2C4D4, 0x4, 'Palico level milestones: bit 0 level 20, bit 1 level 25 (sOtomo +0x4a5a0)', 'DERIVED'),
    ('char', 0x2C4D8, 0x2, 'Dark Piece / Dark Stone counts of the cut Cave feature (sVillage +0x472, +0x473)', 'DERIVED'),
    ('char', 0x2C4DE, 0x4, 'village pet affection, u8 per village, at most 10; 6 unlocks a title word (sVillage +0x3e4)', 'DERIVED'),
    ('char', 0x2C4E2, 0x1, 'Moofah gifts received, at most 10; award 49 (sVillage +0x471)', 'DERIVED'),
    ('char', 0x2C4E3, 0x80, 'village pet names, 4 x char[32] (Moofy, Poogie x 3; sVillage +0x3e8)', 'DERIVED'),
    ('char', 0x2C563, 0x4, 'village pet costumes, u8 per pet, below 40 (sVillage +0x468, pet menu)', 'DERIVED'),
    ('char', 0x2C567, 0x6, 'village pet events seen (u32, bit per village), Housekeeper (u8), start village (u8) (sVillage +0x46c, +0x470, +0x474)', 'DERIVED'),
    ('char', 0x2C6A5, 0x4, 'flags latched from another object (sFlagChecker +0x20, 0x3f43e8)', 'DERIVED'),
    ('char', 0x2C6A9, 0x10, 'sMakeAmulet: 128-bit map over a 107-entry table, tested and set at quest end (0x14c688), read by the quest rotation', 'DERIVED'),
    ('char', 0x2C6B9, 0x4, 'one-time event scenes played, 24 bits (sMakeAmulet +0x74, table 0x15a0c78)', 'DERIVED'),
    ('char', 0x2C67D, 0xD, 'Canteen dishes (sKitchen +0x108; bit = kitchenListMenu record, data/canteen.csv)', 'CONFIRMED'),
    ('char', 0x2C68D, 0xD, 'Canteen dishes viewed (NEW mark cleared), same bits (sKitchen +0x118)', 'DERIVED'),
    ('char', 0xC7A77, 0x1E, 'Guild Card weapon usage, Village', 'CONFIRMED'),
    ('char', 0xC7A95, 0x1E, 'Guild Card weapon usage, Hub', 'CONFIRMED'),
    ('char', 0xC7AB3, 0x1E, 'Guild Card weapon usage, Arena', 'CONFIRMED'),
    ('char', 0xC7AD1, 0x4, 'Guild Card play time (s)', 'DERIVED'),
    ('char', 0xC7AD5, 0x640, 'quest history log, 10 x 160 B', 'DERIVED'),
    ('char', 0xC8115, 0x11, 'Guild Card awards', 'CONFIRMED'),
    ('abs', 0x40, 0x4, 'bonus packs loaded: bit N = privilege pack N (S+0x420, shared)', 'DERIVED'),
    ('abs', 0x44, 0x6, '3 x u16 written only by the transfer 0x52171c, no reader (S+0x3676)', 'DERIVED'),
    ('abs', 0x4A, 0x2, 'title-menu one-time notice shown (S+0x367c), transfer-server link made (S+0x367d)', 'DERIVED'),
    ('abs', 0x4C, 0x1, 'TV brightness: scale 0.4 + 0.025 x value (S+0x367e)', 'DERIVED'),
    ('abs', 0x4D, 0x1, 'rumble on/off (S+0x367f)', 'DERIVED'),
    ('abs', 0xB2A2, 0x2, 'sGameControl +0x5c (no reader) and +0x5d (3DS Circle Pad Pro in use)', 'DERIVED'),
    ('abs', 0xB2A4, 0x1, 'text language, 0 = from the system language (sGameControl +0xa5)', 'DERIVED'),
    ('abs', 0xB2A5, 0x8, 'DLC item packs received, 50 bits (sPrivilege +0xf34)', 'DERIVED'),
    ('abs', 0xB2AD, 0x8, 'DLC Palicoes received, 50 bits (sPrivilege +0xf3c)', 'DERIVED'),
    ('abs', 0xB2B5, 0x4, 'DLC extras received, download type 4: 9 entries; bits 0-3 unlock Guild Card poses (sPrivilege +0xf44)', 'DERIVED'),
    ('abs', 0xB2B9, 0xC, 'DLC extras received, download type 5: 80 Guild Card scenes sold by the Trader (sPrivilege +0xf48)', 'DERIVED'),
    ('abs', 0xB2C5, 0x4, 'DLC extras received, download type 6: 10 pet costumes sold by the Trader (sPrivilege +0xf54)', 'DERIVED'),
    ('abs', 0xB2C9, 0x28, 'DLC extras received, download type 7: 300 title words sold by the Trader (sPrivilege +0xf58)', 'DERIVED'),
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
