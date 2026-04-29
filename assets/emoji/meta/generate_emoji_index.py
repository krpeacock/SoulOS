import re
import json
from pathlib import Path

# Parse emoji-test.txt for fully-qualified emoji
with open('assets/emoji/meta/emoji-test.txt', 'r', encoding='utf-8') as f:
    lines = f.readlines()

emoji_entries = []
emoji_re = re.compile(r'^([0-9A-F ]+);\s*fully-qualified\s*#\s*(.+?)\s+E[0-9.]+\s+(.+)$')
for line in lines:
    m = emoji_re.match(line)
    if m:
        codepoints = m.group(1).strip()
        emoji = m.group(2)
        name = m.group(3)
        hexcode = '-'.join(cp.zfill(4) for cp in codepoints.split())
        emoji_entries.append({
            'hexcode': hexcode,
            'emoji': emoji,
            'name': name,
            'icon': ''
        })

with open('assets/emoji/meta/emoji_list.json', 'w', encoding='utf-8') as f:
    json.dump(emoji_entries, f, ensure_ascii=False, indent=2)

# Build openmoji index: hexcode -> PNG filename.
# FE0F (variation selector) is stripped as a fallback since openmoji omits it.
openmoji_dir = Path('assets/emoji/openmoji-72x72-black')
openmoji_stems = {p.stem for p in openmoji_dir.glob('*.png')}

def find_openmoji_stem(hexcode):
    if hexcode in openmoji_stems:
        return hexcode
    stripped = hexcode.replace('-FE0F', '')
    if stripped in openmoji_stems:
        return stripped
    return None

emoji_index = {}
for entry in emoji_entries:
    stem = find_openmoji_stem(entry['hexcode'])
    emoji_index[entry['hexcode']] = f'{stem}.png' if stem else None

with open('assets/emoji/meta/openmoji_index.json', 'w', encoding='utf-8') as f:
    json.dump(emoji_index, f, ensure_ascii=False, indent=2)

    
