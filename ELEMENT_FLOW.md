  ---
  The two layers

     USER VIEW (recipe / declarative)                  RUNTIME (live arena)

     button(), vstack(), label(), ...   ─── build() ─►   Element + arena entry
                                         (called by framework
                                          at mount time)

  User code never sees Element. It composes recipe structs (Button, Stack, Label, ...). The framework decides when to call Render::build(&tree), which
   is what actually allocates the arena entry.

  ---
  Object graph: building a single <button> from counter_without_macros

     ┌──────────────────────────────────────────────────────────────┐
     │ USER CODE:                                                   │
     │   let view = vstack()                                        │
     │     .child((                                                 │
     │        button()                                              │
     │          .on(click, move |_| count.update(Count::clear))     │
     │          .child("Clear"),                                    │
     │        ...                                                   │
     │     ));                                                      │
     └──────────────────────┬───────────────────────────────────────┘
                            │
                            ▼  builder calls return recipe structs;
                               NOTHING is allocated in any arena yet.

     RECIPE LAYER  (pure data, no Node, no NSView, no arena entry)
     ┌──────────────────────────────────────────────────────────────┐
     │  Stack<(Button, Button, Button, ...)> {  // "vstack" tag     │
     │      tag: "vstack",                                          │
     │      layout:     LayoutAttrs { padding: 16, gap: 12, ... }   │
     │      universal:  UniversalAttrs { ... }                      │
     │      text:       CocoaText      { ... }                      │
     │      decoration: CocoaDecoration { ... }                     │
     │      children:   (Button, Button, Button, ...)               │
     │      ...                                                     │
     │  }                                                           │
     │      │                                                       │
     │      │ each child is itself a recipe:                        │
     │      ▼                                                       │
     │  Button {                                                    │
     │      title:     MaybeReactive::Static("Clear".into())        │
     │      enabled:   None                                         │
     │      handlers:  vec![ PendingHandler::Click(Box<dyn Fn>) ]   │
     │      directives: vec![]                                      │
     │      node_ref:  None                                         │
     │      layout:    LayoutAttrs::default()                       │
     │      universal: UniversalAttrs::default()                    │
     │      text:      CocoaText::default()                         │
     │      decoration: CocoaDecoration::default()                  │
     │      bordered:  None                                         │
     │      ... (other Button-specific fields, all None or default) │
     │  }                                                           │
     └──────────────────────┬───────────────────────────────────────┘
                            │
                            │ mount_to_window("...", size, move || view).run()
                            │   ↓
                            │ framework: open_window() creates a fresh tree,
                            │            then sets content_root + calls
                            │            view.build(&tree).
                            │
                            │ Render::build cascades through each recipe;
                            │ Stack::build calls children.build(tree);
                            │ Button::build is where allocation happens:
                            │
                            │   fn build(self, tree: &TreeRef) -> Self::State {
                            │       let (el, _) = CocoaElement::create_button(tree);
                            │       install(self.title, |t| el.set_title(t));
                            │       for h in self.handlers { h.apply_to(&el); }
                            │       ...
                            │       ElementState { el, _effects, children, ... }
                            │   }
                            │
                            ▼

     RUNTIME LAYER  (this is the same picture as before, but now the path
                     from user code to here is clear)

     ┌──────────────────────────────────────┐
     │  ElementState<(), ()> {              │   Stored by the parent Stack's
     │      el: CocoaElement,               │   state; lives as long as this
     │      _effects: Vec<RenderEffect<()>> │   element is mounted. Drop tears
     │      children: (),                   │   down the el (which decrefs the
     │      _live: LiveTracker,             │   arena entry) plus the effects.
     │  }                                   │
     └─────────────────┬────────────────────┘
                       │
                       ▼
     ┌──────────────────────────────────────┐
     │  Element { node: Node }              │
     └─────────────────┬────────────────────┘
                       ▼
     ┌──────────────────────────────────────┐
     │  Node { inner: SendWrapper<Rc<NodeInner>> } │
     └─────────────────┬────────────────────┘
                       ▼
     ┌──────────────────────────────────────┐
     │  NodeInner {                         │
     │      tree:        TreeRef            │
     │      id:          NodeId             │
     │      view:        Retained<NSView>   │  ← cached for &NSView access
     │      is_borrowed: false              │
     │  }                                   │
     └────┬─────────────────────────────────┘
          │ (tree, id)
          ▼
     ┌────────────────────────────────────────────────────────────┐
     │  LayoutTree<CocoaBackend>          (per-window arena)      │
     │      state: RefCell<SlotMap<NodeData>>                     │
     │      root:  RefCell<Option<NodeId>>                        │
     │      relayout_queued: Cell<bool>                           │
     └────┬───────────────────────────────────────────────────────┘
          │ slotmap[id]
          ▼
     ┌────────────────────────────────────────────────────────────┐
     │  NodeData<CocoaBackend> {                                  │
     │      style, cache, layouts, parent, children               │
     │      refcount:  1                                          │
     │      handlers:  RefCell<NodeHandlers { ... }>  ──┐         │
     │      view:      Retained<NSView>                 │         │
     │      meta:      CocoaMeta { ... }                │         │
     │  }                                               │         │
     └──────────────────────────────────────────────────│─────────┘
                                                        ▼
     ┌────────────────────────────────────────────────────────────┐
     │  NodeHandlers {                                            │
     │      view:           Some(Retained<NSView>)  (back-ref)    │
     │      action_target:  Some(Retained<ActionTarget>) ───┐     │
     │      ...                                             │     │
     │  }                                                   ▼     │
     │                                       ActionTarget {       │
     │                                          ivars: {          │
     │                                             callback:      │
     │                                                Box<FnMut>  │
     │                                                (= the      │
     │                                                 user's     │
     │                                                 |_| count  │
     │                                                 .update    │
     │                                                 closure)   │
     │                                          }                 │
     │                                       }                    │
     │                                          ↑                 │
     │                                          │ target/action   │
     │                                          │ (weak)          │
     │                                       NSButton             │
     └────────────────────────────────────────────────────────────┘

  ---
  Element lifecycle state diagram
  
  This is what an <button> goes through from user code to teardown.

                    (just a value in the user's code — no allocation)
                                      │
                                      │   button()
                                      ▼
                    ┌──────────────────────────────────┐
                    │  RECIPE                          │
                    │     Button { title, handlers,    │
                    │              children, ... }     │
                    │                                  │
                    │  Builder methods (.on, .child,   │
                    │  .padding, ...) consume self and │
                    │  return Self — pure data         │
                    │  manipulation. No arena, no Rc,  │
                    │  no NSView yet.                  │
                    │                                  │
                    │  Composition happens here:       │
                    │  stack(...).child((b1, b2))      │
                    │  produces a tree of recipes.     │
                    └─────────────┬────────────────────┘
                                  │
                                  │  framework: Render::build(&tree)
                                  │    1. CocoaElement::create_button(tree)
                                  │       → NSButton::buttonWithTitle_target_action(...)
                                  │       → Element::from_view → tree.new_leaf → arena entry
                                  │         { refcount: 1, parent: None,
                                  │           style/meta/handlers: defaults }
                                  │       → NodeInner { tree, id, view,
                                  │                     is_borrowed=false }
                                  │    2. apply attrs (each install gets a
                                  │       fresh el.clone() for its closure;
                                  │       the install closures land in
                                  │       ElementState::_effects)
                                  │    3. apply PendingHandlers (e.g. click
                                  │       → ActionTarget allocated, retained,
                                  │       stored in NodeHandlers; setTarget
                                  │       on the NSButton)
                                  │    4. recurse: children.build(tree)
                                  │    5. wrap into ElementState
                                  ▼
              ┌──────────────────────────────────────────────┐
              │  BUILT, ORPHAN                               │
              │     ElementState { el, _effects, children }  │
              │     arena entry { refcount: 1, parent: None }│
              │                                              │
              │     The arena entry exists but isn't         │
              │     reachable from any window root yet.      │
              │     The user's Element handle (held by       │
              │     ElementState.el) is what's keeping the   │
              │     entry alive.                             │
              └──────┬──────────────────┬────────────────────┘
                     │                  │
                     │                  │  ElementState dropped before mount
                     │                  │  (rare; happens if a parent build
                     │                  │   bails out)
                     │                  │  → last Node clone drops
                     │                  │  → NodeInner::Drop → decref → 0
                     │                  │  → arena removes (parent==None)
                     │                  │  → handlers drop, NSView retains
                     │                  │    release
                     │                  ▼
                     │              REMOVED ──────────────────────┐
                     │                                            │
                     │  Mountable::mount(parent_elem, marker):    │
                     │    parent.insert_node(self.el.as_node())   │
                     │    → tree.add_child(parent_id, this_id)    │
                     │    → arena: this.parent = Some(parent_id)  │
                     │    → AppKit:  parent.addSubview(this.view) │
                     │    → schedule_relayout                     │
                     │                                            │
                     │  children.mount(&self.el, None)            │
                     │  (cascades the children's ElementStates)   │
                     ▼                                            │
              ┌──────────────────────────────────────────────┐    │
              │  MOUNTED                                     │    │
              │     parent: Some(...)                        │    │
              │     refcount: 1+ (ElementState's el + any    │    │
              │                   user-held clones)          │    │
              │                                              │    │
              │     Live. Receives:                          │    │
              │       • Taffy layout passes (compute_layout) │    │
              │       • AppKit drawing                       │    │
              │       • events via NSButton's target slot,   │    │
              │         routed to ActionTarget.actionFired:, │    │
              │         which invokes the user closure       │    │
              │                                              │    │
              │     User updates state:                      │    │
              │       • reactive setter fires via            │    │
              │         RenderEffect (lives on _effects)     │    │
              │         → install() closure mutates the el   │    │
              │           (e.g. setStringValue)              │    │
              │         → schedule_relayout if needed        │    │
              │       • OR user toggles attribute / adds a   │    │
              │         child via builder method on the      │    │
              │         underlying el                        │    │
              └────┬──────────────────────┬──────────────────┘    │
                   │                      │                       │
                   │  Mountable::unmount  │  Reactive parent      │
                   │  (parent's Show/Vec/ │  rebuilds away from   │
                   │   Either branch      │  this branch (e.g.    │
                   │   flips away)        │  ErrorBoundary, Show, │
                   │                      │  AnyView::rebuild)    │
                   │                      │                       │
                   │  drop_node(&node)    │  ElementState dropped │
                   │  + removeFromSuperview │  → el drops, effects│
                   │                      │    drop (release      │
                   │                      │    captured Element   │
                   │                      │    clones), children  │
                   │                      │    states drop        │
                   ▼                      ▼                       │
              ┌──────────────────────────────────────────────┐    │
              │  TEARING DOWN                                │    │
              │                                              │    │
              │  ElementState::Drop:                         │    │
              │   1. _effects drops                          │    │
              │      → captured `el.clone()`s drop           │    │
              │      → those Rc<NodeInner>'s decrement       │    │
              │   2. el drops                                │    │
              │      → last Rc<NodeInner> count → 0          │    │
              │      → NodeInner::Drop → tree.decref(id)     │    │
              │   3. children states drop (recurse)          │    │
              │                                              │    │
              │  tree.decref(id):                            │    │
              │   if refcount==0 AND parent==None,           │    │
              │     remove. But here parent might still be   │    │
              │     Some(...) at this point — depends on     │    │
              │     whether parent's remove ran first.       │    │
              │     Either way the entry winds up removed    │    │
              │     once both conditions hold.               │    │
              └────────────────┬─────────────────────────────┘    │
                               ▼                                  │
              ┌──────────────────────────────────────────────┐    │
              │  REMOVED                                     │◄───┘
              │                                              │
              │  Arena entry is gone from the slotmap.       │
              │  NodeData drops outside the state borrow     │
              │  (the hoisted-drop fix in tree.remove);      │
              │  field-drop order fires NodeHandlers::Drop:  │
              │                                              │
              │   1. release text-field/text-view delegate   │
              │      Retaineds explicitly (load-bearing —    │
              │      see NodeHandlers::Drop comment)         │
              │   2. removeTrackingArea                      │
              │   3. disconnect_view_handlers(view)          │
              │      — nils setTarget / setDelegate slots    │
              │   4. field-drop releases ActionTarget +      │
              │      HoverTracker retains; ObjC dealloc      │
              │      fires; LiveTrackers decrement           │
              │   5. NodeData.view drops (one NSView retain  │
              │      released; NodeInner.view holds another) │
              │                                              │
              │  Transitive reachability GC: if this node    │
              │  had any internal-leaf children (e.g.        │
              │  scroll-view documentView wrapper with       │
              │  refcount=0), they orphan + auto-remove.     │
              │                                              │
              │  Mark parent dirty so its next compute_layout │
              │  doesn't reference the stale child entry.    │
              │                                              │
              │  Surviving Node clones (rare — say a user    │
              │  held one alive past unmount) become inert:  │
              │  tree_id still resolves the (stale) id, but  │
              │  arena lookups return None; accessors no-op  │
              │  with defaults.                              │
              └──────────────────────────────────────────────┘
