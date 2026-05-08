// Accessibility permission check.
//
// Querying another app's UI via `AXUIElement` requires the calling
// process to have the "Accessibility" privacy permission. If that's
// not granted, every AX call returns `cannotComplete` and tests
// fail mysteriously. We check upfront and bail with a clear
// remediation message.

import ApplicationServices
import Foundation

public enum Permissions {
    /// Throws a `PermissionError` if the calling process isn't
    /// trusted by the Accessibility subsystem.
    ///
    /// Typically called once from a test's `setUp`. On first run
    /// against an ungranted binary, macOS will show a system
    /// dialog (because we pass `kAXTrustedCheckOptionPrompt:
    /// true`) inviting the user to add the test runner to the
    /// Accessibility list. Subsequent calls are silent.
    public static func requireAccessibilityTrust() throws {
        // Prompt-on-first-call: lets macOS register a TCC entry
        // for `xctest` automatically, which the user can then
        // toggle on in System Settings. Without the prompt, the
        // binary never appears in the list, making the grant step
        // a hunt-and-peck affair.
        let promptKey =
            kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        let opts: CFDictionary = [promptKey: true] as CFDictionary
        let trusted = AXIsProcessTrustedWithOptions(opts)
        guard !trusted else { return }

        // Build a useful diagnostic. The granting target depends on
        // who's running the tests — Terminal, iTerm, Cursor, Claude
        // Code, etc. Each has a separate accessibility entry.
        let runner = ProcessInfo.processInfo.processName
        let parent = parentProcessName() ?? "(unknown parent)"

        throw PermissionError(message: """
            Accessibility permission is not granted to this process \
            (\"\(runner)\"; parent \"\(parent)\").

            UI tests need it to query other apps' AX trees and \
            click their controls.

            Grant it under:
              System Settings → Privacy & Security → Accessibility
              (or run: xcuitests/grant_permission.sh)

            Add the app that's running `swift test` (Terminal, \
            iTerm, Cursor, Claude Code, etc.) and toggle it on. \
            Then re-run.
            """)
    }
}

public struct PermissionError: Error, CustomStringConvertible {
    public let message: String
    public var description: String { message }
}

/// Best-effort parent-process name lookup via `sysctl`. Useful for
/// the diagnostic string — tells the user whether to grant
/// permission to Terminal vs. their IDE.
private func parentProcessName() -> String? {
    let ppid = getppid()
    var info = kinfo_proc()
    var size = MemoryLayout<kinfo_proc>.size
    var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, ppid]
    let res = sysctl(&mib, UInt32(mib.count), &info, &size, nil, 0)
    guard res == 0 else { return nil }

    // p_comm is a fixed-size C-array tuple. Copy bytes out into a
    // [CChar] buffer first so we don't try to take overlapping
    // pointers into the same struct.
    let commTuple = info.kp_proc.p_comm
    let commSize = MemoryLayout.size(ofValue: commTuple)
    var bytes = [CChar](repeating: 0, count: commSize)
    withUnsafePointer(to: commTuple) { tuplePtr in
        tuplePtr.withMemoryRebound(
            to: CChar.self, capacity: commSize
        ) { src in
            bytes.withUnsafeMutableBufferPointer { dst in
                _ = memcpy(dst.baseAddress, src, commSize)
            }
        }
    }
    return String(cString: bytes)
}
