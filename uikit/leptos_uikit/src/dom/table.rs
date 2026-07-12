//! UITableView integration — native sectioned lists whose cells and
//! section headers are leptos-built views.
//!
//! Design: the `<table_view>` element is a **leaf** node (no leptos
//! children). Content is described by a snapshot model —
//! [`TableSection`]s carrying [`TableRow`] builder closures — pushed
//! into the shared [`TableModel`] and re-pushed (followed by
//! `reloadData`) whenever the reactive `sections` attribute re-runs.
//! Rows travel WITH the section counts in one `Vec`, so a UIKit
//! callback racing a model swap can never index newer data with older
//! counts: `cellForRowAtIndexPath:` reads counts and content from the
//! same snapshot.
//!
//! Each visible cell's `contentView` hosts a [`LeptosHostView`]: a
//! UIView subclass owning an independent leptos layout root (the same
//! Owner + build + mount + `compute_layout` recipe as
//! [`crate::dom::navigation::push`], minus the view controller). Cell
//! content is **rebuilt on every dequeue** — the previous [`Hosted`]
//! state (mounted view + reactive `Owner`) is dropped and the row
//! closure is invoked fresh. With ~a dozen visible rows this is cheap,
//! and it sidesteps generic view-recycling entirely.
//!
//! The [`TableDriver`] is one ObjC object implementing both
//! `UITableViewDataSource` and `UITableViewDelegate`. UITableView's
//! dataSource/delegate slots are weak; the strong retain lives on the
//! node's [`IosNodeHandlers`](crate::dom::event::IosNodeHandlers),
//! and `disconnect_view_handlers` nils both slots before that retain
//! drops (same protocol as every other delegate in `event.rs`).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use leptos_native::renderer::view::{Mountable, Render};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{
    define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_foundation::{
    NSIndexPath, NSInteger, NSObjectProtocol, NSSize, NSString,
};
use objc2_ui_kit::{
    NSIndexPathUIKitAdditions, UIScrollViewDelegate, UITableView,
    UITableViewCell, UITableViewCellSelectionStyle, UITableViewDataSource,
    UITableViewDelegate, UIView, UIViewAutoresizing,
};
use reactive_graph::owner::Owner;

use crate::dom::event::LiveTracker;
use crate::dom::layout;
use crate::dom::node::{UikitElem, UikitNodeExt};

// ---------------------------------------------------------------------
// Live counts for leak tests (same convention as event.rs).
// ---------------------------------------------------------------------

static LIVE_TABLE_DRIVERS: AtomicUsize = AtomicUsize::new(0);
static LIVE_TABLE_HOSTS: AtomicUsize = AtomicUsize::new(0);

/// Test-only: live TableDrivers.
#[doc(hidden)]
pub fn table_driver_store_size_for_test() -> usize {
    LIVE_TABLE_DRIVERS.load(Ordering::Relaxed)
}

/// Test-only: live LeptosHostViews.
#[doc(hidden)]
pub fn table_host_store_size_for_test() -> usize {
    LIVE_TABLE_HOSTS.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------
// Public model types — what the app hands to `<table_view sections=…>`
// ---------------------------------------------------------------------

/// Build-and-mount closure: given a fresh layout root, build the view
/// and mount it, returning the boxed state (dropping the box tears the
/// view down — the `ElementState` drop safety-net path, same erasure
/// as `navigation::push`'s `PushedEntry::_state`). This shape instead
/// of `Fn() -> AnyView` because `AnyView` requires `State: Send +
/// Sync`, which a `#[component]`'s opaque `impl IntoView` return
/// doesn't expose; here the view value never crosses a thread — only
/// the closure does.
type BuildInRoot = Arc<dyn Fn(UikitElem) -> Box<dyn Any> + Send + Sync>;

pub(crate) type HeaderBuild =
    Arc<dyn Fn(String, UikitElem) -> Box<dyn Any> + Send + Sync>;

/// One table row: a closure that builds the row's content view. Called
/// once per dequeue of the row's cell (cells rebuild on reuse — see
/// module docs).
pub struct TableRow(BuildInRoot);

impl TableRow {
    pub fn new<F, V>(f: F) -> Self
    where
        F: Fn() -> V + Send + Sync + 'static,
        V: Render<crate::IosBackend>,
        V::State: Mountable<crate::IosBackend> + 'static,
    {
        Self(Arc::new(move |root| {
            let mut state = f().build();
            state.mount(root, None);
            Box::new(state)
        }))
    }
}

/// Erase a `Fn(title) -> view` header builder into the internal
/// build-and-mount shape. Called by the `<table_view>` element
/// builder's `.header(…)`.
pub(crate) fn make_header_build<F, V>(f: F) -> HeaderBuild
where
    F: Fn(String) -> V + Send + Sync + 'static,
    V: Render<crate::IosBackend>,
    V::State: Mountable<crate::IosBackend> + 'static,
{
    Arc::new(move |title, root| {
        let mut state = f(title).build();
        state.mount(root, None);
        Box::new(state)
    })
}

/// One table section: a header title plus its rows. The title feeds
/// either the custom header builder (`<table_view header=…>`) or, when
/// no builder is set, UIKit's default plain-style header.
pub struct TableSection {
    title: String,
    rows: Vec<TableRow>,
}

impl TableSection {
    pub fn new(title: impl Into<String>, rows: Vec<TableRow>) -> Self {
        Self {
            title: title.into(),
            rows,
        }
    }
}

// ---------------------------------------------------------------------
// TableModel — the driver's shared state
// ---------------------------------------------------------------------

/// State shared between the element builder (which owns the reactive
/// `sections` effect) and the [`TableDriver`] (which serves UIKit's
/// callbacks from it). `Rc<RefCell<…>>` — main-thread only, like
/// `TextViewHandlers`.
pub(crate) struct TableModel {
    pub(crate) sections: Vec<TableSection>,
    pub(crate) header_view: Option<HeaderBuild>,
    pub(crate) header_height: Option<f64>,
    /// The `Owner` current when the `<table_view>` was built. Cell and
    /// header views build under children of this owner, so
    /// `expect_context` etc. inside row views resolve against the
    /// table's context chain.
    pub(crate) owner: Owner,
}

pub(crate) type SharedTableModel = Rc<RefCell<TableModel>>;

impl TableModel {
    pub(crate) fn new(
        header_view: Option<HeaderBuild>,
        header_height: Option<f64>,
        owner: Owner,
    ) -> SharedTableModel {
        Rc::new(RefCell::new(TableModel {
            sections: Vec::new(),
            header_view,
            header_height,
            owner,
        }))
    }
}

// ---------------------------------------------------------------------
// Hosted — one mounted leptos view inside a cell / header
// ---------------------------------------------------------------------

/// A built + mounted leptos view living inside a [`LeptosHostView`]:
/// its layout root, its mounted state, and the reactive `Owner` that
/// scopes its signals/effects. Dropping tears all three down in a
/// safe order.
struct Hosted {
    root: UikitElem,
    state: Option<Box<dyn Any>>,
    _owner: Owner,
}

impl Drop for Hosted {
    fn drop(&mut self) {
        // State drops first: the `ElementState` drop safety-net tears
        // the mounted subtree down (effects + store entries + native
        // detach). Then free the layout root's own store entry
        // (nothing else owns it — same explicit `remove` as
        // navigation's `cleanup_popped`). The `Owner` drops last via
        // field order.
        drop(self.state.take());
        self.root.remove();
    }
}

/// Run `build` against a fresh layout root sized to `size`, under a
/// child of `parent_owner`. The recipe from `navigation::push`, minus
/// the view controller.
fn build_hosted(
    build: impl FnOnce(UikitElem) -> Box<dyn Any>,
    parent_owner: &Owner,
    size: NSSize,
    mtm: MainThreadMarker,
) -> Hosted {
    let root = UikitElem::create_container_with(mtm);
    layout::set_flex_direction(root, layout::FlexDirection::Column);
    {
        use leptos_native::renderer::attrs::Dim;
        use leptos_native::renderer::setters;
        setters::set_size_width(root, Dim::Pct(1.0));
        setters::set_size_height(root, Dim::Pct(1.0));
    }

    let owner = parent_owner.child();
    let state = owner.with(|| build(root));

    layout::compute_layout(root, size);

    Hosted {
        root,
        state: Some(state),
        _owner: owner,
    }
}

// ---------------------------------------------------------------------
// LeptosHostView — UIView subclass hosting one leptos layout root
// ---------------------------------------------------------------------

pub(crate) struct HostIvars {
    hosted: RefCell<Option<Hosted>>,
    last_size: Cell<NSSize>,
    _live: LiveTracker,
}

define_class!(
    /// Container for a leptos-built view inside a UITableView cell's
    /// `contentView` (or as a section header). Re-runs the hosted
    /// root's layout whenever UIKit resizes it — cells are dequeued
    /// before their final frame is known, and `layoutSubviews` is the
    /// hook that fires once the real size lands (and again on
    /// rotation / width changes).
    #[unsafe(super(UIView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = HostIvars]
    pub(crate) struct LeptosHostView;

    unsafe impl NSObjectProtocol for LeptosHostView {}

    impl LeptosHostView {
        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };
            let size = self.bounds().size;
            let last = self.ivars().last_size.get();
            if size.width == last.width && size.height == last.height {
                return;
            }
            self.ivars().last_size.set(size);
            if let Ok(hosted) = self.ivars().hosted.try_borrow() {
                if let Some(h) = hosted.as_ref() {
                    layout::compute_layout(h.root, size);
                }
            }
        }
    }
);

impl LeptosHostView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(HostIvars {
            hosted: RefCell::new(None),
            last_size: Cell::new(NSSize::new(0.0, 0.0)),
            _live: LiveTracker::new(&LIVE_TABLE_HOSTS),
        });
        unsafe { msg_send![super(this), init] }
    }

    /// Replace the hosted content: tear down the previous view (if
    /// any), run `build` under a child of `parent_owner`, attach the
    /// new root UIView, and lay it out against the current bounds.
    fn set_content(
        &self,
        build: impl FnOnce(UikitElem) -> Box<dyn Any>,
        parent_owner: &Owner,
    ) {
        let old = self.ivars().hosted.borrow_mut().take();
        drop(old);

        let mtm = MainThreadMarker::new()
            .expect("table cells must be built on the main thread");
        let size = self.bounds().size;
        self.ivars().last_size.set(size);

        let hosted = build_hosted(build, parent_owner, size, mtm);
        self.addSubview(&hosted.root.ui_view());
        *self.ivars().hosted.borrow_mut() = Some(hosted);
    }
}

// ---------------------------------------------------------------------
// TableDriver — UITableViewDataSource + UITableViewDelegate in one
// ---------------------------------------------------------------------

pub struct TableDriverIvars {
    model: SharedTableModel,
    _live: LiveTracker,
}

const CELL_ID: &str = "leptos_cell";

define_class!(
    /// Serves UIKit's table callbacks from the shared [`TableModel`]
    /// snapshot. All row/section lookups are `.get()`-guarded: the
    /// snapshot model makes count/content mismatches structurally
    /// impossible, but a defensive `None` beats an abort under
    /// `panic = "abort"`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TableDriverIvars]
    pub struct TableDriver;

    unsafe impl NSObjectProtocol for TableDriver {}

    unsafe impl UIScrollViewDelegate for TableDriver {}

    unsafe impl UITableViewDataSource for TableDriver {
        #[unsafe(method(numberOfSectionsInTableView:))]
        fn number_of_sections(&self, _tv: &UITableView) -> NSInteger {
            match self.ivars().model.try_borrow() {
                Ok(m) => m.sections.len() as NSInteger,
                Err(_) => 0,
            }
        }

        #[unsafe(method(tableView:numberOfRowsInSection:))]
        fn number_of_rows(
            &self,
            _tv: &UITableView,
            section: NSInteger,
        ) -> NSInteger {
            match self.ivars().model.try_borrow() {
                Ok(m) => m
                    .sections
                    .get(section as usize)
                    .map(|s| s.rows.len() as NSInteger)
                    .unwrap_or(0),
                Err(_) => 0,
            }
        }

        #[unsafe(method_id(tableView:cellForRowAtIndexPath:))]
        fn cell_for_row(
            &self,
            tv: &UITableView,
            index_path: &NSIndexPath,
        ) -> Retained<UITableViewCell> {
            let cell = tv.dequeueReusableCellWithIdentifier_forIndexPath(
                &NSString::from_str(CELL_ID),
                index_path,
            );
            cell.setSelectionStyle(UITableViewCellSelectionStyle::None);
            cell.setBackgroundColor(None);
            let content = cell.contentView();
            content.setBackgroundColor(None);

            let host = find_or_create_host(&content);

            // Clone the row's builder Arc out of the model, then
            // release the borrow before building — building runs
            // arbitrary app view code.
            let built: Option<(BuildInRoot, Owner)> =
                match self.ivars().model.try_borrow() {
                    Ok(m) => m
                        .sections
                        .get(index_path.section() as usize)
                        .and_then(|s| s.rows.get(index_path.row() as usize))
                        .map(|row| (row.0.clone(), m.owner.clone())),
                    Err(_) => None,
                };
            if let Some((build, owner)) = built {
                host.set_content(move |root| build(root), &owner);
            }

            cell
        }

        #[unsafe(method_id(tableView:titleForHeaderInSection:))]
        fn title_for_header(
            &self,
            _tv: &UITableView,
            section: NSInteger,
        ) -> Option<Retained<NSString>> {
            // Only feed UIKit's default header when no custom builder
            // is installed — a custom header view supersedes it.
            match self.ivars().model.try_borrow() {
                Ok(m) if m.header_view.is_none() => m
                    .sections
                    .get(section as usize)
                    .map(|s| NSString::from_str(&s.title)),
                _ => None,
            }
        }
    }

    unsafe impl UITableViewDelegate for TableDriver {
        #[unsafe(method_id(tableView:viewForHeaderInSection:))]
        fn view_for_header(
            &self,
            _tv: &UITableView,
            section: NSInteger,
        ) -> Option<Retained<UIView>> {
            // A fresh host per call — headers aren't pooled. The table
            // releases off-screen headers; dealloc drops the Hosted
            // (state + Owner). Fine for the handful of sections a
            // screen shows.
            let built: Option<(HeaderBuild, String, Owner)> =
                match self.ivars().model.try_borrow() {
                    Ok(m) => match (&m.header_view, m.sections.get(section as usize)) {
                        (Some(builder), Some(s)) => Some((
                            builder.clone(),
                            s.title.clone(),
                            m.owner.clone(),
                        )),
                        _ => None,
                    },
                    Err(_) => None,
                };
            match (built, MainThreadMarker::new()) {
                (Some((build, title, owner)), Some(mtm)) => {
                    let host = LeptosHostView::new(mtm);
                    host.set_content(move |root| build(title, root), &owner);
                    Some(unsafe { Retained::cast_unchecked(host) })
                }
                _ => None,
            }
        }

        #[unsafe(method(tableView:heightForHeaderInSection:))]
        fn height_for_header(
            &self,
            _tv: &UITableView,
            _section: NSInteger,
        ) -> f64 {
            // -1.0 is UITableViewAutomaticDimension — used when the
            // app didn't pin a header height (default UIKit header).
            match self.ivars().model.try_borrow() {
                Ok(m) => m.header_height.unwrap_or(-1.0),
                Err(_) => -1.0,
            }
        }
    }
);

impl TableDriver {
    fn new(model: SharedTableModel, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TableDriverIvars {
            model,
            _live: LiveTracker::new(&LIVE_TABLE_DRIVERS),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// The cell contentView's sole subview is its [`LeptosHostView`];
/// find it on a recycled cell, create + install it on a fresh one.
fn find_or_create_host(content: &UIView) -> Retained<LeptosHostView> {
    for sub in content.subviews().iter() {
        if let Ok(host) = sub.downcast::<LeptosHostView>() {
            return host;
        }
    }
    let mtm = MainThreadMarker::new()
        .expect("table cells must be built on the main thread");
    let host = LeptosHostView::new(mtm);
    host.setFrame(content.bounds());
    host.setAutoresizingMask(
        UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
    );
    content.addSubview(&host);
    host
}

// ---------------------------------------------------------------------
// Install / update entry points (called from the element builder)
// ---------------------------------------------------------------------

/// Create the [`TableDriver`] for `node`'s UITableView, wire it into
/// both the dataSource and delegate slots, and retain it on the node's
/// handler store. No-op if `node` isn't a UITableView.
pub(crate) fn install_table_driver(node: UikitElem, model: SharedTableModel) {
    let Some(tv) = node.try_downcast::<UITableView>() else {
        return;
    };
    let mtm = MainThreadMarker::new()
        .expect("install_table_driver must run on the main thread");
    let driver = TableDriver::new(model, mtm);
    tv.setDataSource(Some(ProtocolObject::from_ref(&*driver)));
    unsafe { tv.setDelegate(Some(ProtocolObject::from_ref(&*driver))) };
    let view_retained = node.ui_view_retained();
    node.with_handlers_mut(|h| {
        h.attach_view(view_retained);
        h.table_driver = Some(driver);
    });
}

/// Swap the model's sections for a fresh snapshot and reload the
/// table. Driven by the `sections` attribute's `RenderEffect`.
pub(crate) fn set_table_sections(
    node: UikitElem,
    model: &SharedTableModel,
    sections: Vec<TableSection>,
) {
    match model.try_borrow_mut() {
        Ok(mut m) => m.sections = sections,
        Err(_) => {
            #[cfg(debug_assertions)]
            eprintln!("[ios_dom] reentrant table sections update skipped");
            return;
        }
    }
    if let Some(tv) = node.try_downcast::<UITableView>() {
        tv.reloadData();
    }
}
