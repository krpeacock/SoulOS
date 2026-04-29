import re
import json

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

# Pre-populate emoji_index.json with names and blank icon fields
emoji_index = {entry['hexcode']: {'name': entry['name'], 'icon': ''} for entry in emoji_entries}
with open('assets/emoji/meta/emoji_index.json', 'w', encoding='utf-8') as f:
    json.dump(emoji_index, f, ensure_ascii=False, indent=2)
