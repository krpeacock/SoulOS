# SoulOS — Design & Architecture Philosophy

SoulOS is a Rust reimagining of PalmOS aimed at obsolete phones and e-readers. Its design deliberately inherits the **Zen of Palm** — the discipline of ruthless simplification that made the original Pilot line feel faster and more intuitive than machines orders of magnitude more powerful. This document is the standing brief for every change to the codebase.

## The Zen of Palm, restated

PalmOS succeeded on 16 MHz CPUs, 160×160 monochrome screens, and 128 KB of RAM not because engineers heroically optimized, but because designers ruthlessly subtracted. The device felt like paper because it refused to feel like a computer.

We inherit that discipline. SoulOS does not aspire to be everything; it aspires to be **enough**, and to be trustworthy, persistent, and immediate.

### The 1-2-3 Rule

Every feature passes three checks, in order:
1. **Identify the specific problem** a real user faces. If you can't name the user and the situation, the problem isn't real.
2. **Find the simplest possible solution.** Usually one action, one screen, one primitive.
3. **Delete everything else.** The hardest step. If it isn't in the 80% path, it doesn't belong in the default path.

This is a filter, not a suggestion.

### The 80/20 Rule

Design for what users do 80% of the time. The 20% — power features, admin, configuration — is welcome, but **discreet**: reachable in two taps, invisible until asked for. The 20% never compromises the layout of the 80%.

### Paper-replacement metaphor

The device is a notebook, not a workstation. Mechanical consequences:

- **Instant on.** Launching an app is state restoration, not cold startup.
- **No save / load.** State is always live. "Save" is a relic of the disk/RAM divide that doesn't apply here.
- **App switching preserves state byte-for-byte.** Mid-sentence, mid-calculation, mid-selection — still true when you return.
- **No splash screens, no loading screens, no login walls** inside the device's own apps. External services may require auth; the app itself does not.

Corollary: *the event loop is the memory model.* An app's in-memory state **is** the app. Persisting it is the platform's job, not the app's.

## Architectural Invariants

### Performance ###
PalmOS had no busy cursor. Everything needs to respond fast enough to not need one.
This needs to be true even on low end hardware like an Esp32. 
Memory is also critical. Assume a very minimal footprint for this software.

### Database-centric storage (no files)

PalmOS had no file system; records were the primitive. We keep this. `soul-db` stores categorized records. Apps do not write files; they mutate records. The platform handles durability, sync, and HotSync-style conflict resolution.

There is no `fopen`. There is `Database::insert`, `get`, `iter_category`, `delete`.

### Event-driven, cooperative single-focus

One foreground app at a time. Apps yield control by returning from `handle`. Background work (network sync, indexing) runs in explicitly-scheduled slots, not ambient threads. Cooperative scheduling is not a limitation; it is the design — it makes the system legible, debuggable, and battery-friendly. Long-running work (a recursion, a network fetch) must poll for break signals, Palm-style (LispMe checked every 1600 SECD steps).

### HAL boundary

`soul-core`, `soul-ui`, `soul-db` are `no_std + alloc`. They compile for any target that implements `soul_hal::Platform`. **Never leak `std` across that line.** That boundary is what lets the same app code run in the desktop simulator and on a bare-metal e-reader.

### Dirty-rect redraw

On e-ink, a full-screen refresh is 300–900 ms and visibly flashes. On old LCDs it wastes battery. The UI layer tracks dirty rectangles and only repaints what changed. This is **required**, not an optimization — it's what makes the e-ink target feasible.

### Segments of focus

The 64 KB segment barrier was an accident of the 68k architecture, but it imposed a discipline: one screen, one form, one unit of user attention, fit in 64 KB. We keep the discipline as a rule: **one app = one concern.** If an app has multiple forms that don't share core state, they are two apps.

### Write-protected by default

Data integrity before ergonomics. Records are immutable from the app's perspective; mutation goes through explicit DB calls the platform can log, sync, and roll back. Slow on purpose — the friction prevents casual corruption.

## Code discipline

- **Favor data over code.** A table of app entries, event kinds, or key mappings beats a switch of match arms.
- **Small primitives.** Button, field, list, menu bar. Not a component framework.
- **No abstractions without three users.** Three callers of the same pattern = a helper. Two = copy.
- **No feature flags, no backward-compat shims, no `TODO(legacy)`.** Delete it or fix it.
- **No comments explaining *what* code does.** Comments only note non-obvious *why* — a hardware quirk, a Palm convention, an intentional constraint.
- **Allocate sparingly.** Target platforms have 2–32 MB RAM. Prefer stack and fixed-size buffers; reach for `alloc::Vec` only when unbounded.

## Non-goals (things we will not build)

- Multitasking UI. No split screens, no picture-in-picture, no "recent apps" drawer.
- Push-notification noise. If notifications exist at all, they live in a single unobtrusive status strip.
- Infinite scroll, engagement loops, telemetry, dark patterns.
- A desktop-class browser. A web view, if ever, is for reading.
- Cloud-only state. The device is sovereign. Sync is optional and explicit.

## What we preserve from 2026

The original Palm was an island. SoulOS isn't. Modern use demands:

- **TLS, UTF-8, real networking** — IMAP, CalDAV, Matrix, ActivityPub, whatever plugs cleanly into the record model.
- **Modern cryptography** (AEAD, Ed25519, Argon2). We do not roll our own; use well-tested libs.
- **Unicode** including RTL and CJK, even on monochrome panels.
- **Color and grayscale displays** as first-class targets alongside 1-bit.

These modernities sit *behind* Palm-style primitives. A mail app is still a Memo-like list of records; its transport is just TLS IMAP instead of HotSync.

## The builder question

PalmOS's most radical users ran Quartus Forth, OnBoard C, or Squeak **on the device** — they treated the handheld as a sovereign workshop, not a consumer terminal. We aspire to the same: SoulOS should eventually be self-hosting enough that a user can author a new app on-device, without a desktop toolchain.

Long-horizon. The near-term implication: keep the app ABI small, stable, and introspectable. No JIT. No reflection. Just a well-documented event loop and HAL.

## Contribution checklist

Before merging, a change should satisfy:

- [ ] Passes the 1-2-3 rule.
- [ ] Fits the 80% path without compromising it.
- [ ] Respects `no_std` boundaries in core crates.
- [ ] Introduces no "loading…" state, splash screen, or explicit save.
- [ ] Preserves app state across switching.
- [ ] Uses dirty-rect redraw, not full-screen clear.
- [ ] Introduces at most one new concept.

## Canonical dimensions

- Virtual screen: 240×320 portrait (3:4). Apps must render correctly here.
- Hard buttons: `Power`, `Home`, `Menu`, `AppA..D`, `PageUp`, `PageDown`. (The four app buttons mirror Palm's Datebook / Address / ToDo / Memo quick-launch.)
- Stylus/touch is the primary input. Keyboards, if present, are a bonus.

## Crate roles

| Crate              | Scope                                            | std? |
| ------------------ | ------------------------------------------------ | ---- |
| `soul-hal`         | Platform trait, input events, hard buttons       | no   |
| `soul-core`        | Event loop, App trait, screen constants          | no   |
| `soul-ui`          | Widget primitives on `embedded-graphics`         | no   |
| `soul-db`          | Record database (Palm Database Manager analogue) | no   |
| `soul-hal-hosted`  | Desktop HAL via SDL2 simulator                   | yes  |
| `soul-runner`      | Desktop binary + built-in apps                   | yes  |

The core four crates must remain `no_std`. That is non-negotiable.

## Tooling preference

Use the rust-analyzer LSP tool for type-checking and diagnostics instead of running `cargo build`. Only run `cargo build` or `cargo check` when you need to verify a full compilation (e.g., checking for linker errors or confirming a build succeeds end-to-end).
