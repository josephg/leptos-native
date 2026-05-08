// Ergonomic Swift wrapper around `AXUIElement`.
//
// The Accessibility framework's API is C-based: lots of
// `AXUIElementCopyAttributeValue(elem, kAXFooAttribute, &out)` and
// `CFBridgingRelease` dance. This file collects the dance into a
// small, typed Swift surface that tests use directly.
//
// Naming convention: methods that find a single descendant by
// some predicate return `AXElement?` (failure = nil). Methods that
// require a hit (`first(...)`, `firstButton(...)`, ...) throw if
// the node isn't there — tests should use the `try` form so
// failures get a useful message rather than an unwrap-nil crash.

import ApplicationServices
import Foundation

public struct AXElement {
    public let raw: AXUIElement

    public init(raw: AXUIElement) {
        self.raw = raw
    }

    // ----------------------------------------------------------------
    // Attribute reads
    // ----------------------------------------------------------------

    /// Generic attribute read. Returns `nil` if the attribute isn't
    /// present (rather than throwing — most callers want optional
    /// semantics, and AX queries on missing attrs are normal).
    public func attribute<T>(_ key: String, as: T.Type = T.self) -> T? {
        var value: CFTypeRef?
        let err = AXUIElementCopyAttributeValue(
            raw, key as CFString, &value
        )
        guard err == .success, let v = value else { return nil }
        return v as? T
    }

    /// Read children. Returns empty array if the element has none /
    /// the attribute is missing.
    public var children: [AXElement] {
        let arr: CFArray? = attribute(kAXChildrenAttribute as String)
        guard let arr = arr else { return [] }
        let count = CFArrayGetCount(arr)
        return (0..<count).map { i in
            // CFArray returns const void * — bridge through unsafe.
            let ptr = CFArrayGetValueAtIndex(arr, i)
            let elem = unsafeBitCast(ptr, to: AXUIElement.self)
            return AXElement(raw: elem)
        }
    }

    // Common attribute shortcuts. `value` is intentionally untyped
    // (`Any?`) since AX attribute values vary — for text fields
    // it's a String, for sliders an NSNumber, for checkboxes an
    // Int (0/1).
    public var role: String? { attribute(kAXRoleAttribute as String) }
    public var subrole: String? { attribute(kAXSubroleAttribute as String) }
    public var title: String? { attribute(kAXTitleAttribute as String) }
    public var identifier: String? {
        attribute(kAXIdentifierAttribute as String)
    }
    public var enabled: Bool {
        (attribute(kAXEnabledAttribute as String, as: NSNumber.self)?
            .boolValue) ?? false
    }
    public var stringValue: String? {
        attribute(kAXValueAttribute as String, as: String.self)
    }
    public var numberValue: NSNumber? {
        attribute(kAXValueAttribute as String, as: NSNumber.self)
    }

    // ----------------------------------------------------------------
    // Attribute writes
    // ----------------------------------------------------------------

    /// Set an attribute value. Returns whether the underlying AX
    /// call reported success — for typing into text fields, prefer
    /// [`setStringValue`].
    @discardableResult
    public func setAttribute(_ key: String, _ value: AnyObject) -> Bool {
        let err = AXUIElementSetAttributeValue(
            raw, key as CFString, value
        )
        return err == .success
    }

    /// Programmatically set a control's value via AX. For an
    /// NSTextField this updates `stringValue` but does NOT fire
    /// `controlTextDidChange:` — AppKit only sends that for user
    /// edits via the field editor. Use [`typeText`] instead when
    /// you want the app's text-change handlers to run.
    @discardableResult
    public func setStringValue(_ s: String) -> Bool {
        setAttribute(kAXValueAttribute as String, s as NSString)
    }

    /// "Type" `s` into a text field by synthesising real keyboard
    /// events via `CGEvent`. Goes through AppKit's normal field-
    /// editor path, so `controlTextDidChange:` and
    /// `controlTextDidEndEditing:` fire naturally — i.e. our
    /// app's `bind:value` write-back leg sees the changes.
    ///
    /// Steps:
    ///   1. Focus this element via AX (`kAXFocusedAttribute`).
    ///   2. For each character, post a CGEvent keyDown + keyUp
    ///      with the character supplied via
    ///      `CGEventKeyboardSetUnicodeString` — bypassing virtual-
    ///      keycode mapping for arbitrary unicode input.
    ///
    /// This is closer to what XCUIAutomation does internally.
    /// The .app must be the foreground process (our `AXSession`
    /// activates it on launch).
    @discardableResult
    public func typeText(_ s: String) -> Bool {
        // Focus the field so keystrokes land here.
        _ = setAttribute(
            kAXFocusedAttribute as String, true as CFBoolean
        )

        // We need a CGEventSource. nil-source events are also
        // valid (post as a "system" source).
        let src = CGEventSource(stateID: .hidSystemState)

        for ch in s {
            guard
                let down = CGEvent(
                    keyboardEventSource: src,
                    virtualKey: 0,
                    keyDown: true
                ),
                let up = CGEvent(
                    keyboardEventSource: src,
                    virtualKey: 0,
                    keyDown: false
                )
            else { return false }

            // Convert the Swift Character into UTF-16 code units
            // (CGEvent's unicode-string API uses UTF-16).
            let utf16: [UniChar] = Array(String(ch).utf16)
            utf16.withUnsafeBufferPointer { buf in
                down.keyboardSetUnicodeString(
                    stringLength: buf.count,
                    unicodeString: buf.baseAddress
                )
                up.keyboardSetUnicodeString(
                    stringLength: buf.count,
                    unicodeString: buf.baseAddress
                )
            }

            // Post to .cghidEventTap so AppKit's normal input path
            // sees them.
            down.post(tap: .cghidEventTap)
            up.post(tap: .cghidEventTap)

            // Tiny pause between keys — AppKit needs a tick to
            // process each event; without it some get coalesced
            // or dropped.
            usleep(2_000)
        }

        return true
    }

    // ----------------------------------------------------------------
    // Actions
    // ----------------------------------------------------------------

    /// Perform an AX action (`kAXPressAction` for buttons,
    /// `kAXIncrementAction` for steppers, etc.).
    @discardableResult
    public func perform(_ action: String) -> Bool {
        let err = AXUIElementPerformAction(raw, action as CFString)
        return err == .success
    }

    /// Click — performs `kAXPressAction` on a button / checkbox /
    /// popup / etc.
    @discardableResult
    public func click() -> Bool {
        perform(kAXPressAction)
    }

    // ----------------------------------------------------------------
    // Tree walking / finders
    // ----------------------------------------------------------------

    /// Recursively walk the AX tree (depth-first, including this
    /// element) yielding every descendant.
    public func descendants() -> [AXElement] {
        var out: [AXElement] = []
        var stack: [AXElement] = [self]
        while let cur = stack.popLast() {
            out.append(cur)
            stack.append(contentsOf: cur.children.reversed())
        }
        return out
    }

    /// First descendant (depth-first, includes self) matching
    /// `predicate`, or `nil` if none.
    public func first(
        where predicate: (AXElement) -> Bool
    ) -> AXElement? {
        descendants().first(where: predicate)
    }

    /// First descendant with the given `kAXRoleAttribute`.
    public func firstChild(role wantRole: String) -> AXElement? {
        first { $0.role == wantRole }
    }

    /// First descendant whose role + visible title both match.
    /// Useful for "the button labelled X".
    public func firstChild(
        role wantRole: String,
        title wantTitle: String
    ) -> AXElement? {
        first { $0.role == wantRole && $0.title == wantTitle }
    }

    /// All descendants of the given role.
    public func allChildren(role wantRole: String) -> [AXElement] {
        descendants().filter { $0.role == wantRole }
    }

    /// All descendants matching role + subrole. Useful for
    /// distinguishing AppKit subclasses that share a role: e.g.
    /// NSSecureTextField has `role=AXTextField, subrole=AXSecureTextField`,
    /// while plain NSTextField has `role=AXTextField` with no
    /// subrole. Pass `subrole: nil` to require an absent subrole.
    public func allChildren(
        role wantRole: String,
        subrole wantSubrole: String?
    ) -> [AXElement] {
        descendants().filter {
            $0.role == wantRole && $0.subrole == wantSubrole
        }
    }

    /// First descendant matching role + subrole (see
    /// [`allChildren(role:subrole:)`]).
    public func firstChild(
        role wantRole: String,
        subrole wantSubrole: String?
    ) -> AXElement? {
        first {
            $0.role == wantRole && $0.subrole == wantSubrole
        }
    }

    // ----------------------------------------------------------------
    // Wait helpers
    // ----------------------------------------------------------------

    /// Poll `predicate` on `self` until it returns true, or until
    /// `timeout` seconds elapse. Useful for waiting on async
    /// reactivity (e.g. a label updating after a click). Polls on
    /// a 25ms cadence.
    @discardableResult
    public func wait(
        timeout: TimeInterval = 2.0,
        for predicate: (AXElement) -> Bool
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if predicate(self) { return true }
            // Short sleep — AX queries are cheap; don't busy-loop.
            usleep(25_000)
        }
        return predicate(self)
    }

    /// Poll until a descendant matching `predicate` exists.
    /// Returns the element or `nil` on timeout.
    public func waitForDescendant(
        timeout: TimeInterval = 2.0,
        matching predicate: @escaping (AXElement) -> Bool
    ) -> AXElement? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let hit = first(where: predicate) { return hit }
            usleep(25_000)
        }
        return first(where: predicate)
    }

    // ----------------------------------------------------------------
    // Diagnostics
    // ----------------------------------------------------------------

    /// Render the AX subtree rooted at `self` as a multi-line
    /// indented string. Intended for diagnosing test failures —
    /// drop a `print(session.window.dumpTree())` and inspect the
    /// real role/subrole/title/value shape AppKit is reporting.
    public func dumpTree(depth: Int = 0) -> String {
        var out = ""
        let pad = String(repeating: "  ", count: depth)
        var bits: [String] = []
        if let r = role { bits.append("role=\(r)") }
        if let s = subrole { bits.append("subrole=\(s)") }
        if let t = title, !t.isEmpty { bits.append("title=\"\(t)\"") }
        if let v = stringValue, !v.isEmpty {
            bits.append("value=\"\(v)\"")
        }
        if let n = numberValue { bits.append("num=\(n)") }
        if let id = identifier, !id.isEmpty {
            bits.append("id=\"\(id)\"")
        }
        out += "\(pad)- \(bits.joined(separator: " "))\n"
        for c in children {
            out += c.dumpTree(depth: depth + 1)
        }
        return out
    }
}
