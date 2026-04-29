# SoulOS Makefile: Common developer tasks

.PHONY: all build emoji-assets hand-crafted-emoji clean check test

all: build

# Generate openmoji_index.json and emoji_list.json from emoji-test.txt + openmoji PNGs.
# Run this after updating openmoji-72x72-black/ or emoji-test.txt.
emoji-assets:
	python3 assets/emoji/meta/generate_emoji_index.py

# Convert hand-crafted PNG emoji (assets/emoji/png/) to raw assets (assets/emoji/raw/).
# Run this after adding a new U+XXXX.png to assets/emoji/png/.
hand-crafted-emoji:
	python3 assets/emoji/convert_emoji_pngs.py

# Build everything; emoji-assets must be up to date before cargo runs.
build: emoji-assets
	cargo build

# Remove generated metadata and hand-crafted raw files (not the openmoji source PNGs).
clean:
	rm -f assets/emoji/raw/*.raw
	rm -f assets/emoji/meta/emoji_list.json assets/emoji/meta/openmoji_index.json

check:
	cargo check

test:
	cargo test
