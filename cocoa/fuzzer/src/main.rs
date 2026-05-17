//! Cocoa correctness fuzzer entry point.
//!
//! Usage:
//!
//! ```sh
//! cargo run --bin cocoa_fuzzer -- --seed 0 --iterations 1000
//! cargo run --bin cocoa_fuzzer -- --seeds 100 --chaos 200
//! ```

use clap::Parser;
use cocoa_dom::{
    app::init_app,
    event::{
        handler_store_size_for_test, text_field_store_size_for_test,
        text_view_store_size_for_test,
    },
    layout, window::open_window, MainThreadMarker,
};
use cocoa_fuzzer::{
    chaos::Chaos,
    compare::compare_trees,
    generator::Generator,
    interact,
    render::build,
    signals::SignalStore,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use reactive_graph::owner::Owner;
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
    /// pass --no-check-leaks to disable.
    #[arg(long, default_value_t = true)]
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
    let res = unsafe {
        objc2::exception::catch(AssertUnwindSafe(|| {
            *slot_ref = Some(f());
        }))
    };
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FullSizes {
    stores: StoreSizes,
    /// Sum of `LayoutTree::node_count()` across both per-window
    /// trees. The fuzzer creates two trees per seed (one for the
    /// reactive mount, one for the static comparison mount).
    /// Pre-mount baseline is the count of any pre-existing trees
    /// from earlier seeds (should be 0 at the start of a clean run
    /// but may grow if a seed leaked).
    tree_nodes: usize,
}

fn main() {
    let args = Args::parse();
    let _ = cocoa_dom::spawner::init();
    let mtm = MainThreadMarker::new()
        .expect("cocoa_fuzzer must run on the main thread");
    let _app = init_app(mtm);

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
        g.generate()
    };

    if args.print_spec {
        println!("=== seed {seed} spec ===\n{spec}");
    }

    let baseline_stores = store_sizes();
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
            state_a.mount(&win_a.content_root, None);
        })?;

        catch_ns("compute_layout-A-1", || {
            layout::compute_layout(win_a.content_root.as_node(), LAYOUT_AVAIL);
        })?;
        pump_run_loop(0.05);

        // Snapshot the post-mount fingerprint. During chaos
        // (signal mutations only — no mounts/unmounts), this
        // fingerprint must stay constant. After teardown, it
        // must return to the pre-seed baseline.
        let post_mount_a = FullSizes {
            stores: store_sizes(),
            tree_nodes: win_a.tree.node_count(),
        };

        // Phase 2: chaos.
        {
            let mut chaos = Chaos {
                rng: &mut rng,
                iterations: args.chaos,
            };
            if args.check_per_iteration {
                let tree_a_for_check = win_a.tree.clone();
                let post_mount_check = post_mount_a;
                let mut per_iter_err: Option<String> = None;
                let mut last_iter = 0usize;
                chaos.run_with_callback(&reactive_store, |iter| {
                    if per_iter_err.is_some() {
                        return;
                    }
                    pump_run_loop(0.005);
                    let now = FullSizes {
                        stores: store_sizes(),
                        tree_nodes: tree_a_for_check.node_count(),
                    };
                    if now != post_mount_check {
                        per_iter_err = Some(format!(
                            "chaos iter {iter}: store/tree drift; \
                             post_mount={:?} now={:?}",
                            post_mount_check, now
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
                interact::drive(win_a.content_root.ns_view(), &mut rng)
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

        layout::compute_layout(win_a.content_root.as_node(), LAYOUT_AVAIL);
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
            state_b.mount(&win_b.content_root, None);
        })?;

        catch_ns("compute_layout-B-1", || {
            layout::compute_layout(win_b.content_root.as_node(), LAYOUT_AVAIL);
        })?;
        pump_run_loop(0.05);

        // Final settling: alternate pump + relayout on both trees
        // until frames stop moving. Shape-changing Show flips can
        // schedule layout passes that propagate over multiple
        // dispatch ticks; the comparison reads frames so we need
        // both trees fully settled.
        for _ in 0..8 {
            pump_run_loop(0.02);
            layout::compute_layout(win_a.content_root.as_node(), LAYOUT_AVAIL);
            layout::compute_layout(win_b.content_root.as_node(), LAYOUT_AVAIL);
        }

        let signal_count = reactive_store.total_count();
        let nodes = spec.size();

        let result = compare_trees(
            win_a.content_root.ns_view(),
            win_b.content_root.ns_view(),
        );

        // Unmount both before dropping windows so the Drop guards
        // see a clean teardown order. Drain the spawner
        // generously so any final RenderEffect tasks complete
        // BEFORE the owner drops — accessing a signal whose
        // owner has been disposed panics.
        catch_ns("unmount-A", || state_a.unmount())?;
        catch_ns("unmount-B", || state_b.unmount())?;
        catch_ns("teardown-A", || win_a.content_root.as_node().teardown())?;
        catch_ns("teardown-B", || win_b.content_root.as_node().teardown())?;
        catch_ns("close-A", || win_a.close())?;
        catch_ns("close-B", || win_b.close())?;
        for _ in 0..10 {
            pump_run_loop(0.02);
        }

        result?;
        // Capture tree sizes before the windows drop (the TreeRef
        // Rc goes away with the OpenedWindow at end of scope).
        // Both trees should report 0 nodes after explicit
        // teardown.
        let tree_a_after = win_a.tree.node_count();
        let tree_b_after = win_b.tree.node_count();
        if tree_a_after + tree_b_after != 0 {
            return Err(format!(
                "tree leak after teardown: tree_a={} tree_b={}",
                tree_a_after, tree_b_after
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
    // Element clones — those holds keep NodeHandlersBundles
    // alive past Owner drop. Without enough pumping the leak
    // counters show false positives.
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
