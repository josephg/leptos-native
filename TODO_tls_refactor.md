# TLS-refactor follow-ups

Captured from the post-refactor review after the cocoa-side
TLS purge (handler stores → ObjC associated objects, PENDING →
per-tree `Cell<bool>`, etc.). See `CLAUDE.md` "Avoid `thread_local!`
for new state" for the design rules; this file is the punch list.

## Priority 1 — iOS handler stores ✅ DONE

Both TLS stores in `uikit/dom/src/event.rs` are gone:

- `HANDLER_STORE` → `HandlerList` ObjC class (wrapping
  `Vec<Retained<ActionTarget>>`) attached as an associated object
  on each host. Holds multiple targets per host (UIControl allows
  multi-target/action, unlike NSControl's single slot).
- `TEXT_VIEW_STORE` → `TextViewDelegate` attached directly as an
  associated object on the UITextView.

Drop_handlers_for + keep_target_alive removed; `Node::teardown` no
longer calls them. Same `attach_action_target<H: AsRef<AnyObject>>`
public surface as cocoa.

LiveTracker / leak-test introspection
(`handler_store_size_for_test`, `text_view_store_size_for_test`)
preserved so the leak-test pattern can move over with the rest of
the iOS test suite when it gets one.

All iOS examples (14 of them) and the broader cocoa test suite
(232 cocoa_dom + 73 leptos_cocoa) still pass.

## Priority 2 — Magic 50 ms pump in `with_scope`

`cocoa/leptos_cocoa/tests/leak_lifecycle.rs::with_scope` pumps the
run loop for 50 ms per test to flush the dispatch-queue spawner
(needed so `bind:value`'s async runner drops its captured Element
clone). With 10 tests that's ~500 ms of pure wait. The value isn't
justified — 10 ms is probably enough for a no-op `dispatch_async`
to land. Tighten once we've measured what the spawner actually
needs. Low risk, low-medium value.

## Priority 2.5 — Residual fuzzer leak in bind:value RenderEffect

The Node-field handler refactor (now landed on both cocoa and iOS,
see `briefing_scroll_overflow.md` / `cocoa_dom::event` module
docs) eliminated the **action handler leak** that the fuzzer was
catching: the OLD design's TLS hashmaps held handlers past Node
drop, and even my associated-objects intermediate held them past
NSView dealloc (because AppKit autorelease/cache retains kept the
NSView alive). The new Node-field design ties handler lifecycle
to Rust `Node` lifecycle — no AppKit interference.

Fuzzer results after the refactor:
- ActionTarget (handler) leak: **dropped ~190 → ~9 over 30 seeds**.
  Almost entirely from a real Rust cycle in `bind.rs`:
  `closure → captured Element → Rc<NodeHandlersBundle> →
  Retained<ActionTarget> → ivars → closure`. Fixed by switching
  the bind action closures to capture `Retained<NSView>` (typed
  to the specific control subclass) instead of `Element`. See
  `bind.rs::install_*_bind` and the `Node::ns_view_retained()`
  helper.
- TextField/TextView delegate leak: still present, but rate is
  unchanged from before the refactor — so the cause isn't the
  cycle that the bind-action fix addressed. Likely a leptos
  reactive-system path holding an Element clone past Owner drop
  (a RenderEffect's captured closure not actually disposed).
  Investigation lead: the bind's *incoming* RenderEffect closure
  still captures `el_for_set: Element` (e.g. bind.rs line 188).
  If that Effect doesn't drop when expected, the Element stays
  alive — and with it the Node, NodeHandlersBundle, and any
  delegates. Hard to triage without leptos-internals knowledge
  but the symptom is now structurally diagnostic ("counter > 0
  means a Node hasn't dropped").

Verified working: cocoa_dom + leptos_cocoa test suites all green
(232 + 73 tests), spotify smoke launches clean, iOS examples
compile.

## Priority 3 — Text-field rebuild appends duplicate handlers

`ensure_text_field_entry` reuses an existing `TextFieldDelegate`'s
`SharedHandlers` Vec and appends a new callback on each install.
If the same field is re-rendered with `bind:value` (or any of the
`on_text_field_*` paths), the OLD callback stays in the Vec and the
NEW one is appended — both fire on each change event.

Pre-existing bug (the old TLS store had the same accumulation),
surfaced by clarifying the lifecycle in the refactor. Fix: have
`bind:value` (and similar reactive installers) swap a per-bind slot
rather than push, or have the SharedHandlers store a single
callback per kind keyed by an installer-supplied token.

Reproduction: bind a signal to a text_field, rebuild the view a
few times, type one character — count how many times the signal
update fires. Currently grows by one per rebuild.

## Priority 4 — DRY ivar bundles

`ActionIvars` / `TextFieldIvars` / `TextViewIvars` are three near-
identical structs each pairing a payload with a `LiveTracker`.
Plus three matching `static AtomicUsize` counters. Could fold to
one `struct Tracked<T> { inner: T, _live: LiveTracker }` and one
counter array indexed by kind, but the duplication is small (3x)
and the next handler type would only save ~15 lines. Defer until
there's a fourth ObjC class to track.

## Priority 5 — Missing focused unit test for `attach_action_target`

The associated-object lifecycle is covered indirectly via leak
tests and the toolbar `drop_releases_action_target` test, but a
focused test in `cocoa/dom/tests/event.rs` would make the
invariant explicit and protect against accidental regressions if
someone touches `ACTION_TARGET_KEY` or the `associate` helper.

Suggested:
- attach + `has_action_target_for_test` returns true
- drop host (inside autoreleasepool + pump) + `has_action_target_for_test` returns false
- second `attach` on same host replaces (verify via `LIVE_ACTION_TARGETS` count stays at 1)

## Priority 6 — `Node::teardown` chain ordering subtlety

`Node::teardown` does `drop_node(self)` then
`view.removeFromSuperview()`. drop_node releases the tree's
`Retained<NSView>` but the Element still holds its own Retained
— so the NSView doesn't deallocate yet (associated objects don't
release yet). Eventually the Element drops, last Retained drops,
NSView dealloc fires, associated objects release.

Correct, but the chain of indirect steps is worth asserting in
a focused test ("after Element drop, associated ActionTarget is
released"). The leak_lifecycle tests verify this transitively
through the counter mechanism — fine for now, but mention this
in a comment near `teardown` if anyone touches that code path.

## Priority 7 — Leak-test isolation across tests

`cocoa/leptos_cocoa/tests/leak_lifecycle.rs` uses a custom
`common::run_tests` runner that runs every test in the same
process, sharing the global `LIVE_*` counters. If a single test
leaks one delegate, every subsequent test's "before" baseline
includes that leak — assertion failures cascade.

Currently all 10 tests pass, so this is latent. A more robust
pattern would have each test snapshot its own pre-call baseline
and compare against it, rather than `assert_eq!(after, baseline)`
where baseline is itself a moving target. Touch when convenient,
not now.

## Priority 8 — `cocoa/fuzzer/*` doesn't build

Unrelated WIP staged in git but the lib doesn't compile (Sync
bound on `dyn ErasedMountable<Dom>`). Pre-existing, not part of
this refactor. Either fix or unstage before it rots further.
