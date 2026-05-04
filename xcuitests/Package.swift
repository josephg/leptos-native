// swift-tools-version: 5.9
//
// End-to-end UI tests for leptos-mac examples.
//
// We don't use XCUIAutomation directly: that requires a "UI test
// bundle" target which SPM doesn't support — only Xcode .xcodeproj
// projects do. Instead, the `AppDriver` library wraps the
// Accessibility framework's `AXUIElement` API into ergonomic Swift
// wrappers, and tests drive the example apps through it.
//
// Net effect is the same: real .app bundles get launched in real
// AppKit windows on the desktop, and tests assert against the live
// accessibility tree (which AppKit auto-populates from NSView
// state).
//
// The test process MUST have Accessibility permission. The first
// run will fail with a clear message; grant the permission to
// whatever runs `swift test` (Terminal / iTerm / Cursor /
// Claude Code / etc.) under System Settings → Privacy & Security →
// Accessibility.

import PackageDescription

let package = Package(
    name: "leptos-mac-uitests",
    platforms: [.macOS(.v11)],
    products: [
        .library(name: "AppDriver", targets: ["AppDriver"]),
    ],
    targets: [
        .target(
            name: "AppDriver",
            path: "Sources/AppDriver"
        ),
        .testTarget(
            name: "LoginFormUITests",
            dependencies: ["AppDriver"],
            path: "Tests/LoginFormUITests"
        ),
        .testTarget(
            name: "SettingsUITests",
            dependencies: ["AppDriver"],
            path: "Tests/SettingsUITests"
        ),
        .testTarget(
            name: "CountersUITests",
            dependencies: ["AppDriver"],
            path: "Tests/CountersUITests"
        ),
    ]
)
