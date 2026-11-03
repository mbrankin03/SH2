#!/usr/bin/env python3
import struct

with open('prototype_extracted/SLUS_202.28', 'rb') as f:
    f.seek(0x3b2b40)
    data = f.read(0x706335)

fund_types = {
    1: ('char', 1),
    2: ('int8_t', 1),
    3: ('uint8_t', 1),
    4: ('int16_t', 2),
    5: ('int16_t', 2),
    6: ('uint16_t', 2),
    7: ('int32_t', 4),
    8: ('int32_t', 4),
    9: ('uint32_t', 4),
    10: ('int32_t', 4),
    11: ('int32_t', 4),
    12: ('uint32_t', 4),
    13: ('void*', 4),
    14: ('float', 4),
    15: ('double', 8),
    16: ('double', 8),
    18: ('void', 0),
    19: ('bool', 1),
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
        if m_name and off_val is not None:
            members.append((off_val, m_name))
        pos += ml
    return size, sorted(members, key=lambda x: x[0])

size, members = get_struct_members('shPlayerWork')
print(f'shPlayerWork size={size}, members={len(members)}')
for i in range(len(members)):
    cur_off, cur_name = members[i]
    next_off = members[i+1][0] if i + 1 < len(members) else size
    span = next_off - cur_off
    print(f'  +0x{cur_off:04x} (size {span:3d}): {cur_name}')
