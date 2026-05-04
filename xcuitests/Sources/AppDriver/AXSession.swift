// Top-level entry: launch an app, get its root AX element, expose
// helpers for the windows underneath it.
//
// One `AXSession` per (test, target-app) — `setUp` creates it,
// `tearDown` terminates the launched app.

import AppKit
import ApplicationServices
import Foundation

public final class AXSession {

    public let app: NSRunningApplication
    public let root: AXElement

    /// Cached reference to the launched app's primary window —
    /// captured when [`init`] returns successfully.
    ///
    /// Why cached: AX queries against the app root return ALL
    /// windows, including system-spawned popups (macOS password
    /// autofill, save-panel sheets, etc.) that aren't part of the
    /// app's own UI tree. Searching `root.allChildren(role:
    /// AXWindow)` on a live test session can return the autofill
    /// popup as `.first` and tests start querying the wrong tree.
    /// Capturing the original window pointer once sidesteps that.
    public let primaryWindow: AXElement

    /// Launch the `.app` at `bundlePath` and wait for its first
    /// window to appear in the AX tree. Throws on permission
    /// denial, launch failure, or window-appearance timeout.
    public init(
        bundlePath: String,
        windowTimeout: TimeInterval = 5.0
    ) throws {
        try Permissions.requireAccessibilityTrust()

        let url = URL(fileURLWithPath: bundlePath)
        let cfg = NSWorkspace.OpenConfiguration()
        cfg.activates = true
        // Promotes a deterministic foreground state so AX queries
        // don't race against initial app activation.
        cfg.addsToRecentItems = false
        cfg.hides = false

        // `openApplication(at:configuration:)` is async on macOS 11+.
        // Block via a dispatch group — tests are synchronous.
        let group = DispatchGroup()
        group.enter()

        var launched: NSRunningApplication?
        var launchErr: Error?
        NSWorkspace.shared.openApplication(at: url, configuration: cfg) {
            (running, err) in
            launched = running
            launchErr = err
            group.leave()
        }
        if group.wait(timeout: .now() + 10.0) == .timedOut {
            throw AXSessionError.launchTimeout(bundlePath: bundlePath)
        }
        if let err = launchErr {
            throw AXSessionError.launchFailed(underlying: err)
        }
        guard let running = launched else {
            throw AXSessionError.launchFailed(
                underlying: NSError(domain: "AppDriver", code: -1)
            )
        }
        self.app = running

        // Build the root AX element. `AXUIElementCreateApplication`
        // doesn't fail; it just returns a handle that won't return
        // useful data until the app finishes setting up its UI.
        self.root = AXElement(
            raw: AXUIElementCreateApplication(running.processIdentifier)
        )

        // Wait for the first window to appear before returning, so
        // tests don't have to babysit the launch boundary.
        let windowReady = root.wait(timeout: windowTimeout) { el in
            !el.allChildren(role: kAXWindowRole as String).isEmpty
        }
        guard windowReady else {
            // Best-effort cleanup before we throw.
            running.terminate()
            throw AXSessionError.windowTimeout(
                bundlePath: bundlePath,
                timeout: windowTimeout
            )
        }

        // Capture the app's first window once and stash it. See
        // `primaryWindow` doc for why caching matters.
        let windows = root.allChildren(role: kAXWindowRole as String)
        guard let first = windows.first else {
            running.terminate()
            throw AXSessionError.windowTimeout(
                bundlePath: bundlePath,
                timeout: windowTimeout
            )
        }
        self.primaryWindow = first
    }

    deinit {
        // Make sure the launched app doesn't outlive its session.
        // tearDown should have called `terminate()` already; this
        // is a fallback.
        if !app.isTerminated {
            app.terminate()
        }
    }

    /// Convenience: returns [`primaryWindow`]. Most single-window
    /// tests use `session.window` as their root.
    public var window: AXElement { primaryWindow }

    /// Launch one of the leptos-mac example apps by env-var key.
    /// Tests typically use this rather than constructing an
    /// `AXSession(bundlePath:)` directly — the example name maps
    /// to a `LEPTOS_MAC_<NAME>_PATH` env var set by
    /// `xcuitests/run_tests.sh`.
    ///
    /// Example: `try AXSession.forExample("LOGIN_FORM")` reads
    /// `LEPTOS_MAC_LOGIN_FORM_PATH`.
    public static func forExample(
        _ envKey: String,
        windowTimeout: TimeInterval = 5.0
    ) throws -> AXSession {
        let var_ = "LEPTOS_MAC_\(envKey)_PATH"
        guard let path = ProcessInfo.processInfo.environment[var_]
        else {
            throw AXSessionError.envVarMissing(name: var_)
        }
        return try AXSession(
            bundlePath: path, windowTimeout: windowTimeout
        )
    }

    /// Force the app to terminate. Test `tearDown` should call
    /// this; the deinit also handles it as a fallback.
    public func terminate() {
        if !app.isTerminated {
            app.terminate()
            // Give it ~250ms to actually exit before yielding.
            // Avoids the next test launching before AppKit fully
            // releases the previous instance.
            for _ in 0..<10 {
                if app.isTerminated { return }
                usleep(25_000)
            }
        }
    }
}

public enum AXSessionError: Error, CustomStringConvertible {
    case launchFailed(underlying: Error)
    case launchTimeout(bundlePath: String)
    case windowTimeout(bundlePath: String, timeout: TimeInterval)
    case envVarMissing(name: String)

    public var description: String {
        switch self {
        case .launchFailed(let err):
            return "AppDriver: failed to launch app — \(err)"
        case .launchTimeout(let path):
            return "AppDriver: launch timed out for \(path)"
        case .windowTimeout(let path, let t):
            return
                "AppDriver: \(path) launched but no window appeared within \(t)s"
        case .envVarMissing(let name):
            return
                "AppDriver: env var \(name) not set — run via run_tests.sh"
        }
    }
}
