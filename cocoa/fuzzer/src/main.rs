//! Cocoa correctness fuzzer entry point.
//!
//! Usage:
//!
//! ```sh
//! cargo run --bin cocoa_fuzzer -- --seed 0 --iterations 1000
//! cargo run --bin cocoa_fuzzer -- --seeds 100 --chaos 200
//! ```

use clap::Parser;
use leptos_native::dom::{app::init_app, event::{
    handler_store_size_for_test, text_field_store_size_for_test,
    text_view_store_size_for_test,
}, layout, spawner, window, window::open_window, MainThreadMarker};
use cocoa_fuzzer::{
    chaos::Chaos,
    compare::compare_trees,
    generator::Generator,
    interact,
    render::build,
    signals::SignalStore,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};
use rand::rngs::ChaCha8Rng;
use rand::SeedableRng;
use reactive_graph::owner::Owner;
use leptos_native::dom::layout::CocoaBackend;
use renderer::LayoutBackend;
use renderer::view::{Mountable, Render};

#[derive(Parser, Debug)]
struct Args {
    /// Run a single fixed seed and exit (mutually exclusive with --seeds).
    #[arg(long)]
    seed: Option<u64>,

    /// Run this many sequential seeds starting at 0.
    #[arg(long, default_value_t = 20)]
    seeds: u64,

    /// Number of chaos mutations per seed.
    #[arg(long, default_value_t = 200)]
    chaos: usize,

    /// Generator depth limit (max nesting).
    #[arg(long, default_value_t = 4)]
    depth: u32,

    /// Reactive-fraction (0.0–1.0).
    #[arg(long, default_value_t = 0.5)]
    reactive: f64,

    /// Print the generated spec for each seed.
    #[arg(long)]
    print_spec: bool,

    /// Stop on the first failure (default true). Set --no-fail-fast
    /// to keep going.
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
    )]
    fail_fast: bool,

    /// Assert thread-local handler / delegate stores return to
    /// their pre-seed baseline after teardown. On by default —
    /// pass `--check-leaks false` to disable.
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
    )]
    check_leaks: bool,

    /// Stricter per-iteration leak check: after every chaos
    /// mutation + pump, store / Taffy-tree sizes must equal the
    /// post-mount snapshot. Catches leaks that grow per signal
    /// write rather than only per mount/unmount cycle. Slower
    /// (synchronous run-loop pump between every mutation) so
    /// off by default.
    #[arg(long, default_value_t = false)]
    check_per_iteration: bool,

    /// XCUI-style interaction mode. After chaos, walk the
    /// mounted NSView tree and trigger every interactable
    /// element via AppKit (`performClick` on NSButtons /
    /// checkboxes, set-stringValue+delegate notify on
    /// NSTextFields). `bind:value` / `bind:checked` write back
    /// to signals so the snapshot reflects post-interaction
    /// state and the static comparison tree should match.
    #[arg(long, default_value_t = false)]
    xcui: bool,

    /// Probability of wrapping a generated node in a `Show`
    /// (shape-changing conditional). Generator default is 0
    /// because the Show plumbing has historically had drain-
    /// ordering issues; opt in to exercise it.
    #[arg(long, default_value_t = 0.0)]
    show_prob: f64,

    /// Probability of emitting a `DynamicList` (length-driven
    /// bulk-rebuild) at each `gen_node` call. Each chaos write
    /// against the count signal rebuilds the whole vstack
    /// subtree — stresses AnyView::rebuild and the mount/unmount
    /// cycle on N copies of a template. 0 = off.
    #[arg(long, default_value_t = 0.0)]
    dynamic_list_prob: f64,

    /// Probability of emitting a `Grid` at each `gen_node`
    /// call. Exercises `<grid columns rows>` + per-cell
    /// `grid_column_at` / `grid_row_at` placement and Taffy's
    /// grid solver. Children may share cells (intentional
    /// collisions) to exercise the solver's overlap handling.
    /// 0 = off.
    #[arg(long, default_value_t = 0.0)]
    grid_prob: f64,

    /// Open this many EXTRA reactive windows per seed (each gets
    /// its own freshly-generated spec, mounted into its own
    /// window under the same `Owner`). Surfaces per-process
    /// state collisions (NSToolbar identifier dedup, NSMenuBar
    /// singleton swaps, NSWindow tab grouping, autosave-key
    /// collisions on shared controls) that single-window seeds
    /// can't reach. Each extra window also goes through the
    /// chaos loop, but doesn't get a comparison rebuild (the
    /// extra windows are stress-only — only the primary
    /// window's reactive vs static tree is diffed). 0 = off.
    #[arg(long, default_value_t = 0)]
    extra_windows: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StoreSizes {
    handler: usize,
    text_field: usize,
    text_view: usize,
}

/// Run `f` and convert any NSException it raises into an `Err`
/// instead of letting it propagate as a foreign-exception abort.
/// AppKit internal-inconsistency exceptions, range exceptions on
/// NSArray/NSToolbar, etc. all bubble through this on macOS.
///
/// `phase` shows up in the error message so the fuzzer report
/// names which call site tripped the exception.
fn catch_ns<R>(
    phase: &'static str,
    f: impl FnOnce() -> R,
) -> Result<R, String> {
    use std::panic::AssertUnwindSafe;
    let mut slot: Option<R> = None;
    let slot_ref = &mut slot;
    let res = objc2::exception::catch(AssertUnwindSafe(|| {
        *slot_ref = Some(f());
    }));
    match res {
        Ok(()) => Ok(slot.expect("catch_ns: f returned without panic")),
        Err(exc) => {
            let (name, reason) = exc
                .as_ref()
                .map(|e| {
                    use objc2::msg_send;
                    use objc2::runtime::AnyObject;
                    use objc2_foundation::NSString;
                    let raw: &AnyObject = e.as_ref();
                    let n: *const NSString =
                        unsafe { msg_send![raw, name] };
                    let r: *const NSString =
                        unsafe { msg_send![raw, reason] };
                    let nstr = if n.is_null() {
                        String::new()
                    } else {
                        unsafe { (*n).to_string() }
                    };
                    let rstr = if r.is_null() {
                        String::new()
                    } else {
                        unsafe { (*r).to_string() }
                    };
                    (nstr, rstr)
                })
                .unwrap_or_default();
            Err(format!("NSException in {phase}: {name}: {reason}"))
        }
    }
}

fn store_sizes() -> StoreSizes {
    StoreSizes {
        handler: handler_store_size_for_test(),
        text_field: text_field_store_size_for_test(),
        text_view: text_view_store_size_for_test(),
    }
}

/// Live entry count of the process-wide thread-local node store.
/// Replaces the old per-window `LayoutTree::node_count()` — every
/// window now shares one store, so this is the total across all open
/// windows. A clean run returns to its pre-seed baseline after
/// teardown.
fn node_count() -> usize {
    CocoaBackend::node_count()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FullSizes {
    stores: StoreSizes,
    /// Live entry count of the shared node store ([`node_count`]) at
    /// snapshot time — covers every open window's subtree. During
    /// shape-stable chaos this stays constant; after teardown it
    /// returns to the pre-seed baseline.
    tree_nodes: usize,
}

fn main() {
    let args = Args::parse();
    let _ = spawner::init().unwrap();
    let mtm = MainThreadMarker::new()
        .expect("cocoa_fuzzer must run on the main thread");
    // The fuzzer doesn't enter the run loop; keep the app +
    // delegate alive for the process's lifetime via plain
    // bindings (NOT mem::forget — the process exits at end of
    // `main()`, so locals drop in scope and we get clean teardown
    // if we ever wanted it). NSApplication is a singleton so its
    // Retained is essentially harmless.
    let (_app, _delegate) = init_app(mtm);

    let mut fails = 0u32;
    let mut runs = 0u32;
    let seeds: Box<dyn Iterator<Item = u64>> = match args.seed {
        Some(s) => Box::new(std::iter::once(s)),
        None => Box::new(0..args.seeds),
    };
    for seed in seeds {
        runs += 1;
        match run_one(seed, &args, mtm) {
            Ok(stats) => {
                println!(
                    "seed {seed:5}  ok   nodes={:3} signals={:3}",
                    stats.nodes, stats.signals
                );
            }
            Err(e) => {
                println!("seed {seed:5}  FAIL  {e}");
                fails += 1;
                if args.fail_fast {
                    break;
                }
            }
        }
    }
    println!("\n{} seed(s) run, {} failures", runs, fails);
    if fails > 0 {
        std::process::exit(1);
    }
}

struct RunStats {
    nodes: usize,
    signals: usize,
}

fn run_one(
    seed: u64,
    args: &Args,
    mtm: MainThreadMarker,
) -> Result<RunStats, String> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Phase 1: generate the spec.
    let spec = {
        let mut g = Generator::new(&mut rng);
        g.max_depth = args.depth;
        g.reactive_fraction = args.reactive;
        g.show_probability = args.show_prob;
        g.dynamic_list_probability = args.dynamic_list_prob;
        g.grid_probability = args.grid_prob;
        g.generate()
    };

    if args.print_spec {
        println!("=== seed {seed} spec ===\n{spec}");
    }

    let baseline_stores = store_sizes();
    let baseline_nodes = node_count();
    let owner = Owner::new();

    // Fixed available size for every layout pass in this seed.
    // Reading the size from the NSView each time would feed back
    // the *previous* layout's height — shape-changing Show flips
    // shrink/grow content, and using the live frame would lock
    // in whichever size was current at first relayout.
    const LAYOUT_AVAIL: objc2_foundation::NSSize =
        objc2_foundation::NSSize {
            width: 800.0,
            height: 600.0,
        };

    let stats = owner.with(|| -> Result<RunStats, String> {
        let reactive_store = SignalStore::new();
        let win_a = catch_ns("open_window-A", || {
            open_window("fuzz-A", (800.0, 600.0), mtm)
        })?;

        // Build + mount reactive tree.
        let view_a = build(&spec, &reactive_store);
        let mut state_a = catch_ns("build-A", || view_a.build())?;
        catch_ns("mount-A", || {
            state_a.mount(win_a.content_root, None);
        })?;

        // Optional: open N extra reactive windows with their own
        // freshly-generated specs (different RNG fork per window).
        // Surfaces per-process collisions the single-window seeds
        // can't reach — see `--extra-windows` docs. Hold the
        // OpenedWindow + state + spec for the duration of the
        // seed so they live alongside `win_a` and exercise multi-
        // window lifetimes.
        struct ExtraWindow {
            // Field declaration order matters — Drop runs in
            // top-down field order, and we need state to unmount
            // before the window closes.
            state: Box<dyn renderer::view::Mountable<leptos_native::Dom>>,
            window: window::OpenedWindow,
            #[allow(dead_code)]
            store: SignalStore,
        }
        let mut extras: Vec<ExtraWindow> = Vec::with_capacity(
            args.extra_windows as usize,
        );
        for i in 0..args.extra_windows {
            let mut sub_rng = ChaCha8Rng::seed_from_u64(
                seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(
                    (i as u64).wrapping_add(1),
                ),
            );
            let extra_spec = {
                let mut g = Generator::new(&mut sub_rng);
                g.max_depth = args.depth;
                g.reactive_fraction = args.reactive;
                g.show_probability = args.show_prob;
                g.dynamic_list_probability = args.dynamic_list_prob;
                g.grid_probability = args.grid_prob;
                g.generate()
            };
            let title = format!("fuzz-extra-{seed}-{i}");
            let win = catch_ns("open_window-extra", || {
                open_window(&title, (640.0, 480.0), mtm)
            })?;
            let extra_store = SignalStore::new();
            let view = build(&extra_spec, &extra_store);
            let mut st: Box<dyn renderer::view::Mountable<leptos_native::Dom>> =
                Box::new(catch_ns("build-extra", || view.build())?);
            catch_ns("mount-extra", || {
                st.mount(win.content_root, None);
            })?;
            catch_ns("compute_layout-extra", || {
                layout::compute_layout(
                    win.content_root,
                    LAYOUT_AVAIL,
                );
            })?;
            extras.push(ExtraWindow {
                state: st,
                window: win,
                store: extra_store,
            });
        }
        pump_run_loop(0.05);

        catch_ns("compute_layout-A-1", || {
            layout::compute_layout(win_a.content_root, LAYOUT_AVAIL);
        })?;
        pump_run_loop(0.05);

        // Snapshot the post-mount fingerprint. During chaos
        // (signal mutations only — no mounts/unmounts), this
        // fingerprint must stay constant. After teardown, it
        // must return to the pre-seed baseline.
        let post_mount_a = FullSizes {
            stores: store_sizes(),
            tree_nodes: node_count(),
        };

        // Phase 2: chaos.
        {
            let mut chaos = Chaos {
                rng: &mut rng,
                iterations: args.chaos,
            };
            if args.check_per_iteration {
                let post_mount_check = post_mount_a;
                // Show / DynamicList legitimately resize the tree
                // and store counts across iterations (a Show flip
                // unmounts the old branch, mounts the new — node
                // counts and handler counts both change). Per-
                // iteration equality is only meaningful when the
                // spec is shape-stable, i.e. when none of the
                // structural-mutation gens are enabled.
                let allow_drift = args.show_prob > 0.0
                    || args.dynamic_list_prob > 0.0;
                let mut per_iter_err: Option<String> = None;
                let mut last_iter = 0usize;
                chaos.run_with_callback(&reactive_store, |iter| {
                    if per_iter_err.is_some() {
                        return;
                    }
                    pump_run_loop(0.005);
                    let now = FullSizes {
                        stores: store_sizes(),
                        tree_nodes: node_count(),
                    };
                    // When structural mutation is enabled, the
                    // post-mount snapshot is the LOWER bound, not
                    // the equality target. We only flag drift if
                    // counts grow unboundedly (a chaos iteration
                    // that doesn't flip anything shouldn't push
                    // counts above the post-mount snapshot's
                    // peak). With drift allowed, switch the check
                    // to "current ≤ 4× post_mount" — a loose
                    // ceiling that still catches per-iteration
                    // leaks (handler count climbing linearly with
                    // chaos iters would blow past this).
                    let bad = if allow_drift {
                        now.stores.handler
                            > post_mount_check.stores.handler * 4 + 8
                            || now.stores.text_field
                                > post_mount_check.stores.text_field * 4 + 8
                            || now.stores.text_view
                                > post_mount_check.stores.text_view * 4 + 8
                            || now.tree_nodes
                                > post_mount_check.tree_nodes * 4 + 8
                    } else {
                        now != post_mount_check
                    };
                    if bad {
                        per_iter_err = Some(format!(
                            "chaos iter {iter}: store/tree drift; \
                             post_mount={:?} now={:?} \
                             (allow_drift={})",
                            post_mount_check, now, allow_drift
                        ));
                    }
                    last_iter = iter;
                });
                let _ = last_iter;
                if let Some(e) = per_iter_err {
                    return Err(e);
                }
            } else {
                catch_ns("chaos", || chaos.run(&reactive_store))?;
            }
        }
        // After chaos, pump generously to let every RenderEffect
        // dispatch settle before we snapshot. Show flips can
        // cascade (the on/off subtree's own reactive bindings
        // re-run when remounted), so we drain in a loop until
        // the run loop reports no more work.
        for _ in 0..20 {
            pump_run_loop(0.05);
        }

        // Phase 2.5 (optional): XCUI-style interaction. Walk
        // the mounted tree and trigger every button/checkbox/
        // text_field via AppKit. `bind:value` / `bind:checked`
        // route the change back to the signal store, so the
        // snapshot below captures the post-interaction state
        // and the static rebuild should match.
        if args.xcui {
            let stats = catch_ns("xcui-drive", || {
                interact::drive(&win_a.content_root.ns_view(), &mut rng)
            })?;
            if args.print_spec {
                println!(
                    "  xcui: clicked {} buttons, typed into {} text fields",
                    stats.buttons_clicked, stats.text_fields_typed,
                );
            }
            for _ in 0..10 {
                pump_run_loop(0.05);
            }
        }

        layout::compute_layout(win_a.content_root, LAYOUT_AVAIL);
        pump_run_loop(0.05);

        // Phase 3: snapshot, then build a parallel static tree.
        let snapshot = reactive_store.snapshot();
        let static_store = SignalStore::new();
        // Pre-seed static_store with the snapshot values so the
        // `build` call's reactive arm initialises signals to the
        // *final* state. We still run `build` with the original
        // `spec` (reactive markers intact) — but every signal will
        // be created with the snapshot's initial, so the static
        // tree mounted below reflects the chaos endpoint.
        for (id, v) in &snapshot.strings {
            static_store.ensure_string(*id, v);
        }
        for (id, v) in &snapshot.bools {
            static_store.ensure_bool(*id, *v);
        }
        for (id, v) in &snapshot.floats {
            static_store.ensure_float(*id, *v);
        }

        let win_b = catch_ns("open_window-B", || {
            open_window("fuzz-B", (800.0, 600.0), mtm)
        })?;
        let view_b = build(&spec, &static_store);
        let mut state_b = catch_ns("build-B", || view_b.build())?;
        catch_ns("mount-B", || {
            state_b.mount(win_b.content_root, None);
        })?;

        catch_ns("compute_layout-B-1", || {
            layout::compute_layout(win_b.content_root, LAYOUT_AVAIL);
        })?;
        pump_run_loop(0.05);

        // Final settling: alternate pump + relayout on both trees
        // until frames stop moving. Shape-changing Show flips can
        // schedule layout passes that propagate over multiple
        // dispatch ticks; the comparison reads frames so we need
        // both trees fully settled.
        for _ in 0..8 {
            pump_run_loop(0.02);
            layout::compute_layout(win_a.content_root, LAYOUT_AVAIL);
            layout::compute_layout(win_b.content_root, LAYOUT_AVAIL);
        }

        let signal_count = reactive_store.total_count();
        let nodes = spec.size();

        let result = compare_trees(
            &win_a.content_root.ns_view(),
            &win_b.content_root.ns_view(),
        );

        // Unmount both before dropping windows so the Drop guards
        // see a clean teardown order. Drain the spawner
        // generously so any final RenderEffect tasks complete
        // BEFORE the owner drops — accessing a signal whose
        // owner has been disposed panics.
        // Tear down extras first (reverse order — Drop usually
        // goes LIFO; mirror it for the AppKit-level NSView /
        // NSWindow lifetimes to expose any close-order
        // dependencies between same-process windows).
        for (idx, mut ex) in extras.into_iter().enumerate().rev() {
            let i = idx as u32;
            catch_ns("unmount-extra", || ex.state.unmount())?;
            catch_ns("teardown-extra", || {
                ex.window.content_root.teardown()
            })?;
            catch_ns("close-extra", || ex.window.close())?;
            // Drop `state` and `store` explicitly to detach them
            // from this scope before the next extra closes.
            drop(ex.state);
            drop(ex.store);
            let _ = i;
        }

        catch_ns("unmount-A", || state_a.unmount())?;
        catch_ns("unmount-B", || state_b.unmount())?;
        catch_ns("teardown-A", || win_a.content_root.teardown())?;
        catch_ns("teardown-B", || win_b.content_root.teardown())?;
        catch_ns("close-A", || win_a.close())?;
        catch_ns("close-B", || win_b.close())?;
        for _ in 0..10 {
            pump_run_loop(0.02);
        }

        result?;
        // The shared node store must return to its pre-seed baseline
        // once every window's content_root has been explicitly torn
        // down. Under the NodeId-over-thread-local-store model,
        // teardown cascades the whole structural subtree out of the
        // store immediately — no refcount, no deferred sweep — so a
        // non-baseline count here is a real leak.
        let nodes_after = node_count();
        if nodes_after != baseline_nodes {
            return Err(format!(
                "node store leak after teardown: {} live nodes \
                 (baseline {})",
                nodes_after, baseline_nodes
            ));
        }
        let _ = win_a;
        let _ = win_b;
        Ok(RunStats { nodes, signals: signal_count })
    });

    // Drop the owner explicitly so its disposers fire before the
    // next seed. Pump generously so the bind:value RenderEffect
    // async tasks drain (they hold Element clones in their
    // captured closures; without draining, TeardownGuards stay
    // alive past `state.unmount()`).
    drop(owner);
    // Drain extensively so dispatch-queue async tasks (e.g.
    // bind:value's RenderEffect runner) drop their captured
    // Element clones — those holds keep NodeInner Rc counts
    // (and therefore arena handler entries) alive past Owner
    // drop. Without enough pumping the leak counters show false
    // positives.
    objc2::rc::autoreleasepool(|_| {
        for _ in 0..20 {
            pump_run_loop(0.02);
        }
    });

    let stats = stats?;
    if args.check_leaks {
        let after = store_sizes();
        if after != baseline_stores {
            return Err(format!(
                "LEAK after teardown: baseline={:?} after={:?}",
                baseline_stores, after
            ));
        }
    }
    Ok(stats)
}

/// Drain pending dispatch-queue tasks (RenderEffect runs are
/// scheduled via `DispatchQueue::main().exec_async`). Without
/// pumping, effects don't fire and the reactive tree stays at the
/// state from the previous compute.
fn pump_run_loop(seconds: f64) {
    let rl = NSRunLoop::mainRunLoop();
    let limit = NSDate::dateWithTimeIntervalSinceNow(seconds);
    unsafe { rl.runMode_beforeDate(NSDefaultRunLoopMode, &limit) };
}
