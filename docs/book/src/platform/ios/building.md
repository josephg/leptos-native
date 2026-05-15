# Building and Launching

iOS examples don't use an Xcode project. The bundled
`run_ios.sh` script handles the whole build → bundle → install
→ launch flow with plain `cargo` + `xcrun simctl`.

## What the script does

Each example crate ships its own `run_ios.sh`. The flow is the
same in all of them:

1. **Build** with `cargo build --target aarch64-apple-ios-sim`
   (or `x86_64-apple-ios` on Intel hosts).
2. **Bundle** — copy the built binary into a hand-rolled
   `.app` directory with an `Info.plist`:
   - `LSRequiresIPhoneOS=true`
   - `MinimumOSVersion=15.0`
   - `UIApplicationSceneManifest` with
     `UIApplicationSupportsMultipleScenes=false`
   - Empty `UILaunchScreen` dict (so the OS uses modern device
     sizing automatically).
3. **Find or boot a simulator** — `xcrun simctl list devices`
   to find a running iPhone simulator, or boot one if none is
   running.
4. **Terminate any prior install**, then `xcrun simctl install`
   the fresh bundle.
5. **Launch** via `xcrun simctl launch --console`, which streams
   the app's stdout to your terminal.

## Interactive use

```sh
cd uikit/examples/counter
./run_ios.sh
```

The simulator window appears with the app running. Quit the
simulator (Cmd-Q) or kill the script (Ctrl-C) to exit.

## Non-interactive use (`-t SECONDS`)

For CI, agents, or just "verify it didn't crash on launch":

```sh
./run_ios.sh -t 3
```

The script auto-terminates the app after 3 seconds and returns
control. The stdout stream is still printed.

```admonish warning
Without `-t`, `run_ios.sh` blocks until you manually kill the
app. Always pass `-t` from any automated context — agents,
shell scripts, CI pipelines — or the script will hang
indefinitely.
```

## Sharing the build cache

iOS example crates aren't workspace members (Cargo doesn't
support target-conditional members), so by default each example
gets its own `target/` directory — wasteful when you're
iterating on the framework and rebuilding many examples.

`run_ios.sh` sets `CARGO_TARGET_DIR` to the shared workspace
`target/`:

```sh
CARGO_TARGET_DIR=$(repo_root)/target cargo build ...
```

If you build outside `run_ios.sh`, set this manually:

```sh
CARGO_TARGET_DIR=$(pwd)/target cargo build \
  --manifest-path uikit/examples/counter/Cargo.toml \
  --target aarch64-apple-ios-sim
```

Otherwise you'll end up with `target/` directories scattered
across `uikit/examples/*/`.

## Type-checking without building

```sh
cargo check -p ios_dom      --target aarch64-apple-ios-sim
cargo check -p leptos_uikit --target aarch64-apple-ios-sim
```

Useful for iterating on the framework crates without paying the
cost of a full link.

## Targeting real devices

`run_ios.sh` only handles the simulator. For real-device builds:

1. Add the device target: `rustup target add aarch64-apple-ios`.
2. Build with `--target aarch64-apple-ios`.
3. Sign and deploy via `ios-deploy` or by wrapping the build
   into a real Xcode project.

Real-device deployment is outside the scope of this fork's
tooling — you'll need to manage signing, provisioning, and
distribution through the standard Apple toolchain.
