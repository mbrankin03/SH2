#!/usr/bin/env python3
import struct

with open('prototype_extracted/SLUS_202.28', 'rb') as f:
    f.seek(0x3b2b40)
    data = f.read(0x706335)

fund_types = {
    1: 'char',
    2: 'int8_t',
    3: 'uint8_t',
    4: 'int16_t',
    5: 'int16_t',
    6: 'uint16_t',
    7: 'int32_t',
    8: 'int32_t',
    9: 'uint32_t',
    10: 'int32_t',
    11: 'int32_t',
    12: 'uint32_t',
    13: 'void*',
    14: 'float',
    15: 'double',
    16: 'double',
    18: 'void',
    19: 'bool',
}

def parse_attrs(chunk):
    attrs = {}
    i = 6
    while i < len(chunk):
        if i + 2 > len(chunk): break
        attr, = struct.unpack_from('<H', chunk, i)
        i += 2
        form = attr & 0xf
        if form in (1, 2, 6):
            val, = struct.unpack_from('<I', chunk, i)
            i += 4
        elif form == 3:
            blen, = struct.unpack_from('<H', chunk, i)
            i += 2
            val = chunk[i:i+blen]
            i += blen
        elif form == 4:
            blen, = struct.unpack_from('<I', chunk, i)
            i += 4
            val = chunk[i:i+blen]
            i += blen
        elif form == 5:
            val, = struct.unpack_from('<H', chunk, i)
            i += 2
        elif form == 7:
            val = chunk[i:i+8]
            i += 8
        elif form == 8:
            end = chunk.find(b'\x00', i)
            if end != -1:
                val = chunk[i:end].decode('latin1', errors='replace')
                i = end + 1
            else:
                val = chunk[i:].decode('latin1', errors='replace')
                i = len(chunk)
        else:
            break
        attrs[attr] = val
    return attrs

def get_struct_members(name):
    target = name.encode() + b'\x00'
    idx = data.find(target)
    if idx == -1: return None
    p = idx - 1
    while p > 0 and p > idx - 30:
        l, t = struct.unpack_from('<IH', data, p)
        if t == 0x13 and p + l > idx: break
        p -= 1
    s_attrs = parse_attrs(data[p:p+l])
    sib = s_attrs.get(0x12, p + l)
    size = s_attrs.get(0xb6, 0)
    
    pos = p + l
    members = []
    while pos < sib and pos < len(data):
        ml, mt = struct.unpack_from('<IH', data, pos)
        if ml < 4: break
        m_attrs = parse_attrs(data[pos:pos+ml])
        m_name = m_attrs.get(0x38, None)
        loc = m_attrs.get(0x23)
        off_val = None
        if loc and len(loc) >= 5 and loc[0] == 4:
            off_val = struct.unpack_from('<I', loc, 1)[0]
        fund = m_attrs.get(0x55)
        user_type = m_attrs.get(0x72)
        mod_user = m_attrs.get(0x83)
        mod_fund = m_attrs.get(0x65)
        if m_name and off_val is not None:
            members.append((off_val, m_name, fund, user_type, mod_user, mod_fund))
        pos += ml
    return size, sorted(members, key=lambda x: x[0])

size, members = get_struct_members('shPlayerWork')

lines = []
lines.append("/* Automatically generated exact C layout for shPlayerWork (PlayerState) */")
lines.append("typedef struct PlayerState {")

for i in range(len(members)):
    cur_off, cur_name, fund, user_t, mod_u, mod_f = members[i]
    next_off = members[i+1][0] if i + 1 < len(members) else size
    span = next_off - cur_off
    
    # Decide type declaration
    if span == 16 and (fund == 14 or 'pos' in cur_name or 'rot' in cur_name or 'angle' in cur_name or 'vec' in cur_name or 'revise' in cur_name):
        decl = f"    float     {cur_name}[4];"
    elif span == 16 and cur_name == 'player':
        decl = f"    void*     {cur_name}; uint8_t _pad_player[12];"
    elif span == 12 and (fund == 14 or 'pos' in cur_name or 'rot' in cur_name):
        decl = f"    float     {cur_name}[3];"
    elif span == 8 and fund == 14:
        decl = f"    float     {cur_name}; float _pad_{cur_name};"
    elif span == 8 and 'rot_y' in cur_name:
        decl = f"    float     {cur_name}[2];"
    elif span == 4:
        if fund == 14:
            decl = f"    float     {cur_name};"
        elif fund in (7, 8):
            decl = f"    int32_t   {cur_name};"
        elif fund in (9, 10, 11, 12):
            decl = f"    uint32_t  {cur_name};"
        elif fund == 13 or mod_u or mod_f:
            decl = f"    void*     {cur_name};"
        elif cur_name in ('target', 'look_tgt', 'enemy_liedown'):
            decl = f"    void*     {cur_name};"
        else:
            decl = f"    uint32_t  {cur_name};"
    elif span == 2:
        if fund in (4, 5):
            decl = f"    int16_t   {cur_name};"
        elif fund == 6:
            decl = f"    uint16_t  {cur_name};"
        else:
            decl = f"    uint16_t  {cur_name};"
    elif span == 1:
        if fund == 2:
            decl = f"    int8_t    {cur_name};"
        else:
            decl = f"    uint8_t   {cur_name};"
    elif span == 3:
        decl = f"    uint8_t   {cur_name}; uint8_t _pad_{cur_name}[2];"
    elif span == 6:
        decl = f"    uint16_t  {cur_name}; uint8_t _pad_{cur_name}[4];"
    else:
        decl = f"    uint8_t   {cur_name}[{span}];"
    lines.append(f"{decl:46} /* +0x{cur_off:04x} */")

lines.append("} PlayerState;")

print("\n".join(lines[:60]))
print("...")
print("\n".join(lines[-20:]))

with open('include/sh2/player_state_gen.h', 'w') as f:
    f.write("\n".join(lines) + "\n")
