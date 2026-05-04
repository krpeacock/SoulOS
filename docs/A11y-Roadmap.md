# SoulOS Accessibility Roadmap

## Context

The Gemini research (mobile a11y survey, 2025–2026) names a four-attribute canonical accessibility node — **name, role, state, value** — and identifies the patterns that separate functional screen-reader support from frustrating support: complete semantic state, focus traps in modals, controllable speech rate, verbosity levels, screen curtain, and an Item Chooser / rotor for fast navigation in dense screens.

SoulOS already has the *plumbing* for a screen reader (`A11yManager`, `pending_speech` drain in the event loop, `Platform::speak`, triple-press Home toggle, focus highlight rendering) but the *content model* is anemic: `A11yNode { bounds, label, role: String }` with no `state` or `value`, free-form role strings, partial widget coverage (Launcher returns `vec![]`), and `soul-ui::form::A11yHints` declared in JSON but never read by the runtime. Two parallel a11y types coexist in `crates/soul-core/src/a11y.rs:5-8` (old `AccessibleNode { text }`) and `:11-16` (new `A11yNode`), which is exactly the trap that lets coverage rot.

This roadmap stages the upgrade so each phase is a single new concept, ships independently, and respects the Zen of Palm constraints (no_std core, dirty-rect required, 2–32 MB RAM, e-ink redraw cost, single-focus app model). Generative image descriptions, multitasking, audio ducking, and Braille HID are deliberately out of scope.

## Phase 1 — Canonical a11y node: state + value, full widget coverage

**New concept:** the four-attribute a11y node (`name`, `role`, `state`, `value`).

**Problem.** Focusing a checkbox today says `"Notify, checkbox"` — the user cannot tell if it is checked. A slider says `"Volume, slider"` — no percent. The Launcher's `a11y_nodes()` returns empty, so the home screen is invisible to a screen reader. Without the data, every later phase (rotor, item chooser, verbosity composition) is built on sand.

**Scope (in).**
- Replace `A11yNode` with `{ bounds, label, role: A11yRole, state: A11yState, value: Option<String> }`.
- Convert `role` to a small enum with a `Custom(String)` escape hatch — finite roles let the rotor/chooser filter without string compare.
- Wire every widget: Launcher (one node per icon), TextArea, TextInput (split label from value), Keyboard (per-key nodes), Scrollbar (`value = "{pct}%"`), Checkbox (`state.checked`), and `soul-ui::form::Form::a11y_nodes` (read `properties.checked` / `properties.text`, map `ComponentType` → `A11yRole`).
- Update `speak_focused` in `soul-runner/src/lib.rs` to compose `name → role → state → value` via a single `A11yNode::utterance(&self) -> String`.
- Delete the legacy `AccessibleNode { text }` and `Accessible` trait — replaced entirely by `App::a11y_nodes`.
- Extend the harness with `find_state(label, A11yState)`, `find_value(label) -> Option<String>`, and tighten `find_role` to take `A11yRole`.

**Scope (out).** Anything touching `Platform::speak`. Focus model changes. Verbosity / curtain / rate.

**Code touches.**
- `crates/soul-core/src/a11y.rs` — new types + `utterance()`; delete `AccessibleNode`, `Accessible`.
- `crates/soul-core/src/lib.rs` — bump `App::a11y_nodes` doc; no signature change.
- `crates/soul-runner/src/lib.rs:660` (`speak_focused`) — call `node.utterance()`.
- `crates/soul-runner/src/launcher.rs:447` — emit one node per app icon.
- `crates/soul-runner/src/calculator.rs:385` — `value = Some(self.display_text())` on display node.
- `crates/soul-ui/src/form.rs:291` — populate state/value from `properties`; map `ComponentType` → `A11yRole`.
- `crates/soul-ui/src/textinput.rs:297` — split label from value.
- `crates/soul-ui/src/scrollbar.rs` — emit `ScrollBar` node with percent value.
- `crates/soul-hal-hosted/src/harness.rs:484-510` — new query helpers.
- `crates/soul-a11y/src/lib.rs` — keep mirror in sync or delete; pick one source of truth.

**Acceptance.**
- Harness: `find_role(A11yRole::Checkbox, "Notify").unwrap().state.checked == Some(true)` after toggle.
- Harness: `find_role(A11yRole::Slider, "Volume").unwrap().value == Some("70%".into())`.
- Harness: `find_role(A11yRole::Button, "Calculator").is_some()` from Launcher (today fails).
- `harness.coverage_report()` reports zero un-labelled tappable rects across all built-in apps.
- Existing `harness_a11y_*` tests still pass.

**Risks.** State + value + role-enum together is borderline "one new concept" — justified because role-enum without it forces every callsite to be touched twice. Binary cost ~30 bytes/node × <30 nodes/screen = negligible. JSON forms keep working: `role: "button"` parses to `A11yRole::Button`, unknown strings → `Custom`. Zero e-ink redraw impact (data-only).

---

## Phase 2 — Focus traversal as a first-class concept

**New concept:** the `FocusRing` — an ordered, scoped view over the active app's a11y tree that owns the focus index and traversal rules.

**Problem.** Focus state is scattered today: `Host::a11y_focus` keeps an index, `A11yManager::focus_index` is unused, and wraparound math is duplicated in `soul-runner/src/lib.rs:947-957`. When Phase 4+ adds modals (date picker, confirm-delete), focus will leak behind the modal — the user swipes right, focus jumps to a button hidden under the popup, they activate it, lose data.

**Scope (in).**
- New `FocusRing { nodes, index, scope }` and `FocusScope { Whole, Modal { rect } }` in `soul-core::a11y`.
- `FocusRing::next/prev/current/rebuild`. `rebuild` preserves index by `(label, role)` match if the node still exists.
- Rebuild once per frame after `app.handle` returns, gated by a cheap dirty hash (node-count + first/last labels) to avoid e-ink work on idle frames.
- New `App::a11y_focus_scope(&self) -> Option<Rectangle>` with default `None`. The runtime filters the ring to nodes whose bounds intersect that rect.
- Delete `Host::a11y_focus` and the inline wraparound logic.

**Scope (out).** Granularity / rotor (Phase 5). Touch event hijacking — stylus events still flow as today; we are scoping the *focus view*, not redirecting input.

**Code touches.**
- `crates/soul-core/src/a11y.rs` — `FocusRing`, `FocusScope`, replace `focus_index` with `focus: FocusRing`.
- `crates/soul-core/src/lib.rs` — call `ctx.a11y.focus.rebuild(app.a11y_nodes(), app.a11y_focus_scope())` after each `app.handle`.
- `crates/soul-runner/src/lib.rs:517-958` — delete `a11y_focus`, route `speak_focused`/`activate_focused` through `FocusRing`.

**Acceptance.**
- Harness: `focus_next()` / `focus_prev()` return next/previous nodes.
- Test: an app reporting `a11y_focus_scope = Some((0,0,200,100))` causes `focus_next` to return only nodes inside that rect.
- Test: state change that preserves a node by `(label, role)` keeps focus on it; if removed, falls back to index 0.
- No regression: PageUp/PageDown cycle Calculator/Builder identically to today.

**Risks.** Per-frame rebuild cost — mitigated by the dirty hash. `App::a11y_focus_scope` defaults to `None`, so existing apps need no changes.

---

## Phase 3 — Speech pipeline at the HAL: rate, interrupt, verbosity, screen curtain

**New concept:** the `SpeechRequest` — a structured TTS request (text + rate + interrupt + punctuation) replacing the bare string passed to `Platform::speak`.

**Problem.** macOS `say` uses the system default rate (~175 wpm). Linux/Web `speak()` is a `println!` stub — no audio. Rapid focus stepping queues five full sentences and the SR falls minutes behind. On e-ink, the focus highlight redraws on every step, costing 300–900 ms of visible flash. **Recommendation: split into 3a → 3c → 3b** (rate first because Linux/Web are unusable today; curtain second because it's a tiny PR with the e-ink win; verbosity last because it depends on Phase 1's structured hints being widely populated).

**Scope (in).**
- `Platform::speak(&mut self, req: SpeechRequest<'_>)` — `SpeechRequest { text: &str, rate_wpm: u16, interrupt: bool, punctuation: Punctuation }`. Lives in `soul-hal`; `&str` + primitives only, no `String`, no_std-safe.
- Linux: shell out to `espeak-ng -s {wpm}` if present; warn-once otherwise.
- Web: `window.speechSynthesis.speak(...)` via `web-sys` (wasm-only cfg).
- Android: JNI `TextToSpeech.setSpeechRate` + `speak`.
- macOS: `say -r {wpm}`; track child PID, `kill` on `interrupt: true`.
- `Platform::set_screen_curtain(&mut self, on: bool)` with default no-op. On e-ink: suppress flushes (saves the flash and battery — this is the win, more than privacy). On hosted: black framebuffer. Toggle by long-press Power while a11y is on.
- `Verbosity { Low, Medium, High }` on `A11yManager`, composes the utterance string at `speak()` time:
  - Low: name only.
  - Medium: name + role-when-not-obvious + value.
  - High: name + role + state + value + hint (from `A11yHints`).

**Scope (out).** Per-app overrides (Phase 4). Audio ducking. Sound effects. Braille HID. Generative descriptions.

**Code touches.**
- `crates/soul-hal/src/lib.rs:80` — replace `speak` signature; add `set_screen_curtain`.
- `crates/soul-hal-hosted/src/lib.rs:326` — spawn `say`/`espeak-ng`, store `Child`, kill-on-interrupt, `try_wait` to reap.
- `crates/soul-runner-web/src/platform.rs:252` — `speechSynthesis` integration.
- `crates/soul-hal-android/src/platform.rs:373` — JNI bridge.
- `crates/soul-hal-hosted/src/harness.rs` — `speech_log: Vec<SpeechRequest>` (record full requests, not just text).
- `crates/soul-core/src/a11y.rs` — `Verbosity`, `Punctuation`; verbosity composition in `speak_node`.
- `crates/soul-core/src/lib.rs:410-413` — drain loop builds `SpeechRequest` from current rate/verbosity.

**Acceptance.**
- Harness: `speech_log()[i].rate_wpm` and `.interrupt` are assertable.
- Test: rapid `focus_next() × 5` produces 5 entries, all `interrupt: true` after the first.
- Test: at Low, focusing a checkbox produces `"Notify"`; at Medium, `"Notify, checked"`; at High, `"Notify, checkbox, checked. {hint}"`.
- Test: `set_screen_curtain(true)` blanks the framebuffer to 0 within one frame on hosted; `flush()` still called.

**Risks.** No `std`-leak: `SpeechRequest` is `&str` + primitives. Shell-out and JNI live only in their respective platform crates. PID tracking needs `try_wait` to reap zombies. Splitting into 3a/3b/3c keeps the "one concept" rule defensible.

---

## Phase 4 — Per-app a11y settings persisted in soul-db

**New concept:** the `system_settings` database — one well-known soul-db database, scoped by app ID for overrides.

**Problem.** Phase 3 ships hardcoded defaults. Users cannot save preferences. The temptation is a JSON file on disk; that violates the database-centric storage rule. PalmOS-style category-per-namespace already gives us the per-app scope.

**Scope (in).**
- One `system_settings` `Database`, opened by `Host::new`. Records: `category = app_id_hash` (or 0 for global), `data = bincode { key: u8, value: SettingValue }`.
- Fixed key space: `SR_RATE_WPM`, `SR_VERBOSITY`, `SR_SCREEN_CURTAIN`, `SR_PUNCTUATION` (room for ~250 more u8 tags).
- `Host::new` reads global settings → applies to `A11yManager`. On app switch, re-read with that app's category for overrides.
- New scripted Settings app (`assets/scripts/settings.rhai`) — Form with slider (rate), three radios (verbosity), checkbox (curtain). Uses existing form `binding` mechanism. Wired into `APP_MANIFEST` in `soul-runner/src/lib.rs:179`.
- Thin wrapper `crates/soul-runner/src/system_settings.rs` (~80 LoC).

**Scope (out).** Sync. Reset wizard (the form has a Reset button). Generic key-value API on `soul-db` — resist generalizing for one user.

**Acceptance.**
- Test: write rate=240, restart `Host`, observe `speech_log()[0].rate_wpm == 240`.
- Test: per-app override — set rate=320 with `category = hash("address")`; in Address app focus → 320; back at Launcher → 240.
- Test: Settings slider activated by tap persists across harness sessions.

**Risks.** u8-tagged keys → cheap forward-compat (ignore unknown). Settings-as-script means it gets Phase 1's a11y for free and is editable in Builder.

---

## Phase 5 — Item Chooser overlay + rotor (granularity filter)

**New concept:** the **Item Chooser** — a modal overlay listing every focusable node on the current screen with substring search and role filter; jumps focus on selection.

**Problem.** A Builder canvas with 40+ widgets means 39 swipes from index 0 to "Save". Sighted users navigate by sight; SR users need search. Item Chooser is the Palm-graffiti analogue of the VoiceOver Rotor — one high-leverage primitive that subsumes both.

**Scope (in).**
- New `crates/soul-runner/src/item_chooser.rs` — App impl: snapshot of `Vec<A11yNode>` at open, plus `query: String`, `selected: usize`. Renders a TextInput at top, scrollable filtered list below.
- Trigger: Menu hard button when a11y is on (every gesture has a button alternative — research mandate).
- Rotor: a four-segment toggle in the same overlay — "All / Buttons / Headings / Form fields" — sets `RotorMode` on `A11yManager` which filters `FocusRing::next/prev`.
- The overlay is just a `NativeKind::ItemChooser` app pushed onto the stack — gets focus traps for free from Phase 2.
- Harness: `open_item_chooser`, `chooser_filter`, `chooser_select`.

**Scope (out).** Fuzzy matching (substring is enough at <50 nodes). Persisted rotor choice (easy add later via Phase 4). Voice search.

**Acceptance.**
- Test: Builder form with "Save" at index 38 — `open_item_chooser → chooser_filter("Save") → chooser_select` lands focus on Save in one frame.
- Test: with `RotorMode::Headings`, `focus_next` cycles only `Heading` nodes.
- Test: closing without selecting restores prior focus.

**Risks.** Full-screen redraw on open (~400 ms e-ink flash) is acceptable as deliberate user action. ~5 KB peak memory for 40-node snapshot. If reviewers reject 5-as-one-concept, split: 5a chooser, 5b rotor segment.

---

## Critical Files

- `crates/soul-core/src/a11y.rs` — types, `FocusRing`, `Verbosity`, `RotorMode`.
- `crates/soul-core/src/lib.rs` — event loop integration (focus rebuild, speech drain).
- `crates/soul-hal/src/lib.rs:80` — `Platform::speak` signature, `set_screen_curtain`.
- `crates/soul-hal-hosted/src/lib.rs:326` — desktop TTS shell-out.
- `crates/soul-hal-hosted/src/harness.rs:484-510` — query helpers, `speech_log`.
- `crates/soul-ui/src/form.rs:291` — `Form::a11y_nodes` reading `properties`.
- `crates/soul-ui/src/{textinput,textarea,scrollbar,keyboard}.rs` — per-widget node emission.
- `crates/soul-runner/src/lib.rs` — `Host::a11y_focus` removal, settings load on startup, item-chooser dispatch.
- `crates/soul-runner/src/{launcher,calculator}.rs` — node coverage.
- `crates/soul-runner/src/{system_settings,item_chooser}.rs` — new modules.
- `assets/scripts/settings.rhai` — new scripted Settings app.

## Sequencing

| Phase | Concept | Depends on | Indep. ship |
|---|---|---|---|
| 1 | Four-attribute node | — | Yes |
| 2 | FocusRing + scope | Phase 1 nodes | Yes |
| 3a | Rate + interrupt at HAL | Phase 1 utterance | Yes |
| 3b | Screen curtain | — | Yes |
| 3c | Verbosity composition | Phase 1 hints | Yes |
| 4 | system_settings DB | Phase 3 knobs | Yes |
| 5 | Item Chooser + rotor | Phase 2 ring | Yes |

Each phase is ~200–800 LoC of net change. No phase pushes `std` into `no_std` core: TTS shell-outs and `web-sys` calls live only in their HAL crates; the data crossings (`SpeechRequest`, `A11yNode`, `FocusRing`) are `&str` + primitives + alloc.

## Verification

End-to-end:
1. Run `cargo test -p soul-hal-hosted` — harness a11y query tests pass.
2. Run the desktop simulator, triple-press Home, navigate Launcher with PageUp/PageDown — every app icon announces.
3. Open Calculator, press buttons — display value announced on change.
4. Open Builder form with a checkbox, toggle it — checkbox `state.checked` flips in `speech_log`.
5. (After Phase 3) Set `SR_RATE_WPM = 240`, hear faster speech; long-press Power, screen blanks, no flash on focus step.
6. (After Phase 4) Restart simulator — settings persist.
7. (After Phase 5) Open a 40-widget Builder form, press Menu, type "Save", press Enter — focus lands on Save.

## Out of Scope (Documented Non-Goals)

- Generative image descriptions (Apple Intelligence / Gemini Nano analogue) — wrong RAM budget, wrong philosophy.
- Multitasking, recents drawer, split screens.
- Braille HID — long-horizon; re-evaluate after Phase 5.
- Audio ducking — defer until users ask; OS-API-heavy on each platform.
- Live regions — the per-frame rebuild in Phase 2 effectively gives "live polite" announcements when `(label, value)` changes; no new primitive needed.
- Touch-exploration (drag finger to read) — stylus is primary; spatial exploration is awkward without a finger. Phase 2 + Phase 5 cover the same need.
