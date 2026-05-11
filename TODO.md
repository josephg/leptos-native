# TODO

- Testing
  - Reactivity fuzz tests
  - Complex app layout examples
    - Apple Pages
    - iOS settings app
    - Spotify
    - Discord
    - ???
- Features
  - Grid layout
  - Global overrides for default font and stuff?
  - Spacer component ("view"?)
  - All the Cocoa properties
  - Error boundary?
  - Tokio / etc runtime integration + examples
  - Menu / MenuItem
  - Mac: Sane way to bundle a binary into an .app
  - Mac: App icon?
  - Mac: DocumentView
  - iOS: UINavigationController / stack
  - Linux: App icon?
- Big features
  - Android support?
  - Windows support?
- Dev tooling
  - Chrome introspection protocol
  - Hot module reloading
- Deployment
  - Clean up git / github
  - Website
  - Rename -> Pachys
  - Attribute level documentation
  - Layout documentation
  - 
  



## Old stuff

- core examples
    - [ ] slots 
    - [ ] hackernews
    - [ ] counter\_isomorphic
    - [ ] todo\_app\_sqlite
- reactivity 
    - Signal wrappers
    - SignalDispose implementations on all Copy types
    - untracked access warnings
- ErrorBoundary
    - [ ] Separate component?
- Suspense/Transition components?
- callbacks
    - unsync StoredValue
- \_meta package (and use in hackernews)
- integrations
- update tests
- hackernews example
  - TODOs
  - Suspense/Transition/Await components
  - nicer routing components
  - async routing (waiting for data to load before navigation)
  - `<A>` component
  - figure out rebuilding issues: list (needs new signal IDs) vs. regular rebuild
