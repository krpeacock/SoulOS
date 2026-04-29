# SoulOS Makefile: Common developer tasks

.PHONY: all emoji-assets clean

all:
	cargo build

# Compile emoji PNGs to .raw assets for embedding
emoji-assets:
	cd assets/emoji && python3 convert_emoji_pngs.py

# Remove all generated .raw files and index
clean:
	rm -f assets/emoji/*.raw assets/emoji/meta/emoji_index.json

# Build everything (including emoji assets)
build: emoji-assets
	cargo build

# Run tests
check:
	cargo check

test:
	cargo test
