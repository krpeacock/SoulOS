# Harness — a test driver for SoulOS

Status: design
Owner: platform
Last updated: 2026-04-24

## 1. The problem

An app author wants to know, before shipping a build to an e-reader, that **tapping at (120, 200) in Notes after typing "hello" lands the cursor on the right character**. Today they have two options:

1. Boot the runner, move a real mouse, squint at pixels.
2. Write a `TestScenario` in `crates/soul-hal-hosted/src/testing.rs`, fire it through `--test`, observe nothing — `screenshot()` is a stub and there are no assertions.

Neither is adequate for a codebase that aspires to run on a bare-metal e-reader where you can't attach a debugger. We need a way to ask the running system:

- *Is "hello" on screen right now?*
- *Which widget is at (120, 200)?*
- *Does the current frame match the golden image?*

…and to answer it in a CI-runnable, deterministic, pure-Rust test.

## 2. The solution — one sentence

`Harness` — a single struct that owns a headless `Platform`, steps the event loop frame-by-frame under a virtual clock, injects input, and answers queries against the app's a11y tree and framebuffer.

## 3. What we delete (the 1-2-3 rule, applied)

We do **not** introduce:

- A DSL, JSON test format, or external script runner. Tests are ordinary `#[test]` Rust functions.
- A new crate. `Harness` lives in `crates/soul-hal-hosted/src/harness.rs`, replacing the thin `testing.rs`.
- A widget-tree introspection API. `App::a11y_nodes()` already exists and returns labeled rectangles; that **is** the semantic view. If a widget isn't accessible, it isn't testable — and that is the right incentive.
- Draw-call tracing, OCR, or a render spy. The framebuffer is the ground truth; the a11y tree is the semantic map. Two primitives, no third.
- A network protocol, daemon, or socket. The harness links the runner as a library and drives it in-process. (If someone ever needs Python control, a JSON-RPC shell is additive and can be designed then, not now.)
- Retries, waits-for-selector, implicit timing. Tests advance frames explicitly. No hidden polling.

One new concept. `Harness`. That's it.

## 4. Architecture

```
┌───────────────────────────────────────────────────────────┐
│  #[test] fn notes_types_hello()                           │
│    let mut h = Harness::new();                            │
│    h.launch("notes");                                     │
│    h.type_text("hello");                                  │
│    h.settle();                                            │
│    assert!(h.find_text("hello").is_some());               │
│    h.snapshot("notes_hello");                             │
└──────────────────────────┬────────────────────────────────┘
                           │ calls
                           ▼
┌───────────────────────────────────────────────────────────┐
│  Harness  (crates/soul-hal-hosted/src/harness.rs)         │
│  ┌─────────────────────┐  ┌──────────────────────────┐    │
│  │  HeadlessPlatform   │  │  Host (runner's App)     │    │
│  │  - MiniFbDisplay    │  │  - full app registry     │    │
│  │  - VecDeque<Event>  │  │  - a11y_nodes()          │    │
│  │  - VirtualClock     │  │                          │    │
│  └─────────────────────┘  └──────────────────────────┘    │
│         tick() = one pass of the soul-core event loop     │
└───────────────────────────────────────────────────────────┘
```

### 4.1 `HeadlessPlatform`

A second `Platform` impl alongside `HostedPlatform`. Same `MiniFbDisplay` framebuffer, same input queue — but:

- No minifb window. Nothing opens, nothing blocks.
- `now_ms()` reads from a `VirtualClock` owned by the harness, not the OS.
- `sleep_ms(n)` advances the virtual clock by `n` and returns immediately.
- `flush()` is a no-op.
- `speak(s)` appends to a `Vec<String>` the harness can assert against.

### 4.2 Frame stepping

`Harness::tick()` runs exactly one iteration of `soul_core::run`'s inner loop:

1. Drain pending events → `Host::handle()`.
2. Emit `Event::Tick(clock_ms)`.
3. If dirty is non-empty, `Host::draw()` into `MiniFbDisplay`.
4. Advance `VirtualClock` by 16 ms (configurable).

Because the clock is virtual, 1000 ticks of animation take microseconds.

### 4.3 `settle()`

Repeatedly `tick()` until `Ctx::dirty` has been empty for N consecutive frames (default 2). Hard cap at 120 ticks to prevent infinite-animation tests from hanging. Returns `Err(SettleTimeout)` on cap.

This is the one implicit wait we permit. It replaces all ad-hoc `sleep`s.

## 5. The API

```rust
pub struct Harness { /* HeadlessPlatform + Host + clock */ }

impl Harness {
    // ── Lifecycle ──
    pub fn new() -> Self;                        // empty DB
    pub fn with_db(db: Database) -> Self;        // seed for fixtures
    pub fn launch(&mut self, app_id: &str);      // enters the named app
    pub fn home(&mut self);                      // HardButton::Home

    // ── Input ──
    pub fn tap(&mut self, x: i16, y: i16);       // down + up, 1 tick apart
    pub fn drag(&mut self, from: (i16, i16), to: (i16, i16), steps: u8);
    pub fn press(&mut self, b: HardButton);      // down + up
    pub fn key(&mut self, k: KeyCode);
    pub fn type_text(&mut self, s: &str);        // one KeyCode::Char per tick

    // ── Frame control ──
    pub fn tick(&mut self);                      // advance exactly one frame
    pub fn advance_ms(&mut self, ms: u32);       // tick until virtual clock elapsed
    pub fn settle(&mut self) -> Result<(), SettleTimeout>;

    // ── Semantic queries (over a11y tree) ──
    pub fn find_text(&self, needle: &str) -> Option<A11yNode>;
    pub fn find_role(&self, role: Role, label: &str) -> Option<A11yNode>;
    pub fn nodes(&self) -> Vec<A11yNode>;        // whole tree
    pub fn tap_node(&mut self, node: &A11yNode); // taps node.bounds.center()

    // ── Pixel queries ──
    pub fn pixel(&self, x: i16, y: i16) -> Gray8;
    pub fn framebuffer(&self) -> &MiniFbDisplay;
    pub fn save_png(&self, path: impl AsRef<Path>) -> io::Result<()>;

    // ── Golden images ──
    pub fn snapshot(&self, name: &str);          // panics on mismatch
    pub fn speech_log(&self) -> &[String];       // for a11y tests
}
```

### Why this shape

- Linear, synchronous, no futures. Reads top-to-bottom like a Palm procedure.
- No fluent chains, no implicit `await`. Every frame the runner advances is visible in the test.
- Queries return `Option<A11yNode>`, not booleans. The test decides whether missing means "fail" or "wait another tick".
- `tap_node` closes the loop: find → act → observe, in three lines.

## 6. Introspection model — why a11y is enough

`A11yNode { bounds, label, role }` already exists (`crates/soul-a11y/src/lib.rs`). It was built for screen readers, but it is exactly the view a test driver wants: **a list of rectangles that a human can talk about.**

Consequence: making an app testable is the same work as making it accessible. The two concerns collapse into one. That is the Palm move — delete the distinction that didn't need to exist.

Any widget that does not appear in `a11y_nodes()` is invisible to the harness *and* to a screen-reader user. The fix in both cases is the same: emit an `A11yNode` for it. We will not add a test-only introspection channel.

## 7. Screenshots & golden images

- Format: PNG (8-bit grayscale, 240×320).
- Encoder: `png` crate, dev-dependency only. Zero cost in release builds.
- Golden path: `tests/snapshots/<name>.png`, committed. Missing → write on first run and fail with a message saying so.
- Diff: byte-equal after PNG decode. No fuzzy thresholding in v1; fonts are deterministic on our target. If we ever need a tolerance, add it when a real case demands it.
- `UPDATE_SNAPSHOTS=1 cargo test` regenerates. Reviewers diff the PNGs in the PR.

## 8. Determinism

Three sources of non-determinism, all pinned:

| Source | Today | Under Harness |
|---|---|---|
| Wall clock | `Instant::now()` in `HostedPlatform::now_ms` | `VirtualClock`, advanced only by `tick()` / `advance_ms()` |
| Sleep | `std::thread::sleep` | Advances the virtual clock, returns immediately |
| Input timing | minifb poll cadence | Events dequeued one per tick |

Animations driven by `Event::Tick(ms)` become reproducible. A tap at frame 7 lands at frame 7, always.

RNG and network are out of scope — apps that need them should take them as dependencies and tests should inject deterministic seeds / fakes at that layer.

## 9. CI integration

- `cargo test -p soul-hal-hosted` runs all harness tests.
- No display server needed (no minifb).
- Parallel-safe: each `Harness` owns its framebuffer and DB; no shared state.
- Snapshots in `tests/snapshots/` are reviewed in PRs like any other diff.

## 10. Non-goals

- Cross-process automation of the real runner binary. (If ever needed, wrap this same `Harness` in a JSON-RPC server — additive.)
- Multi-touch, gestures beyond drag. Palm was single-stylus; so are we.
- Fuzzing / property tests. The harness doesn't prevent them but also doesn't invent a framework for them — plain `proptest` on top works.
- Record/replay of real user sessions. Possible later by logging the minifb event stream; not v1.
- Recording videos. Snapshot at key frames; that is enough.

## 11. Migration of existing scaffolding

These go away or fold in:

- `crates/soul-hal-hosted/src/testing.rs` — replaced by `harness.rs`. `TestScenario` and `TestEvent` builders are deleted; their content becomes ordinary calls on `Harness`.
- `test_automation.py`, `test_soulos.py`, `validate_scripts.py` at repo root — deleted. macOS-specific `screencapture` + `cliclick` wrappers were stopgaps for the missing harness.
- `run_headless_test` in `soul-runner/src/main.rs` (lines 1077–1124) — deleted. The `--test <scenario>` flag goes with it. Tests live in `#[test]` functions, not runner flags.
- `scenarios::*` (build_todo_app, verify_todo_app, test_notes_app, …) — rewritten as `#[test]` fns in `crates/soul-runner/tests/`.

Net: **~200 lines deleted, ~400 added, one concept replaced by one concept.**

## 12. Implementation plan

Stages, each individually mergeable:

1. **`HeadlessPlatform` + `VirtualClock`.** Stands up a second HAL, no public API yet. Proves determinism.
2. **`Harness::{new, launch, tick, tap, press, key, type_text}`.** Minimal input + stepping. Port one existing scenario (`test_notes_app`) to a `#[test]` fn and confirm it passes.
3. **A11y queries (`find_text`, `find_role`, `tap_node`).** Requires auditing existing apps for `a11y_nodes()` coverage; gaps are filed as separate issues.
4. **PNG snapshots (`save_png`, `snapshot`).** Golden-image workflow.
5. **`settle()`, `advance_ms()`, `speech_log()`.** Polish.
6. **Delete `testing.rs`, the Python scripts, `--test` flag, and `run_headless_test`.**

Checkpoint after stage 2: can we write a Notes test that is shorter and clearer than the current `test_notes_app` scenario? If not, the API is wrong — revise before stage 3.

## 13. Contribution checklist (from CLAUDE.md)

- [x] 1-2-3 rule — problem (no assertable tests), solution (`Harness`), deletion (testing.rs, Python scripts, `--test`).
- [x] Fits the 80% path — ordinary `cargo test`, no new tools.
- [x] Respects `no_std` — harness lives in `soul-hal-hosted`, never touches `soul-core` / `soul-ui` / `soul-db` / `soul-hal` internals beyond their public traits.
- [x] No loading state, splash, or save concept introduced.
- [x] Preserves app state across switching — the harness *tests* that, it doesn't break it.
- [x] Dirty-rect redraw preserved — `settle()` reads `Ctx::dirty` rather than forcing a full redraw.
- [x] One new concept: `Harness`.

## 14. Open questions

- **A11y tree coverage.** Some existing apps likely under-report nodes. Do we block the harness on closing every gap, or ship the harness and file coverage tickets per app?
  - Proposed: ship at stage 3 with a `harness::coverage_report()` helper that lists tappable regions with no corresponding a11y node, so gaps are discoverable.
- **Text input into non-focused widgets.** `type_text` assumes a focused text field. Do we auto-tap into a field first, or require the test to?
  - Proposed: require explicit `tap` first. Magic is anti-Palm.
- **Golden-image flakiness on font rendering.** `embedded-graphics` font rendering is deterministic, but if it ever changes, every snapshot breaks at once.
  - Proposed: treat a font-version bump as an intentional mass-regeneration event, same as updating a CSS baseline. Not a reason to add fuzzy matching.
