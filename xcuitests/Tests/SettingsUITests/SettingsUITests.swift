// End-to-end UI tests for `examples/settings_macos`.
//
// Exercises slider drag, mute checkbox gating slider.enabled, and
// pop_up_button selection.

import AppDriver
import ApplicationServices
import XCTest

final class SettingsUITests: XCTestCase {

    var session: AXSession!

    override func setUpWithError() throws {
        continueAfterFailure = false
        session = try AXSession.forExample("SETTINGS")
    }

    override func tearDown() {
        session?.terminate()
        super.tearDown()
    }

    // ----------------------------------------------------------------
    // Diagnostic — run once when we add new tests to see the AX
    // tree shape. Disabled via name prefix; rename to enable.
    // ----------------------------------------------------------------
    func disabled_test_dump_tree() {
        print(session.window.dumpTree())
    }

    // ----------------------------------------------------------------
    // Initial state
    // ----------------------------------------------------------------

    func test_window_present() {
        XCTAssertEqual(session.window.title, "Settings")
    }

    func test_initial_slider_present() {
        XCTAssertNotNil(
            session.window.firstChild(role: kAXSliderRole as String),
            "slider should be present at launch"
        )
    }

    func test_initial_volume_label_shows_50_percent() {
        let label = session.window.waitForDescendant(timeout: 1.0) {
            ($0.stringValue ?? "").hasSuffix("%")
        }
        XCTAssertEqual(
            label?.stringValue, "50%",
            "volume label should start at 50%"
        )
    }

    func test_initial_mute_unchecked() {
        XCTAssertEqual(muteCheckbox().numberValue?.intValue, 0)
    }

    func test_initial_theme_label_shows_system() {
        let label = session.window.waitForDescendant(timeout: 1.0) {
            ($0.stringValue ?? "").hasPrefix("Selected theme:")
        }
        XCTAssertEqual(
            label?.stringValue,
            "Selected theme: System",
            "theme label should default to System"
        )
    }

    // ----------------------------------------------------------------
    // Slider drag → volume label updates
    // ----------------------------------------------------------------

    /// Setting the slider's AX value fires NSSlider's target/action
    /// (which our `bind:value` outgoing leg listens to via
    /// `Element::on_action`). This pushes the new value into the
    /// volume signal, which updates the label.
    func test_slider_drag_updates_volume_label() {
        let slider = sliderElement()
        XCTAssertTrue(slider.setAttribute(
            kAXValueAttribute as String, NSNumber(value: 75.0)
        ))

        let label = session.window.waitForDescendant(timeout: 1.0) {
            $0.stringValue == "75%"
        }
        XCTAssertNotNil(
            label, "volume label should reflect slider value (75%)"
        )
    }

    // ----------------------------------------------------------------
    // Mute checkbox gates slider.enabled + label flips to "Muted"
    // ----------------------------------------------------------------

    func test_mute_disables_slider_and_changes_label() {
        muteCheckbox().click()

        // Wait for slider.enabled to flip false (the `enabled=
        // move || !mute.get()` reactive gate).
        let s = sliderElement()
        XCTAssertTrue(
            s.wait(timeout: 1.0) { !$0.enabled },
            "slider should disable when mute is on"
        )

        // Label flips from "X%" to "Muted".
        let label = session.window.waitForDescendant(timeout: 1.0) {
            $0.stringValue == "Muted"
        }
        XCTAssertNotNil(
            label, "volume label should read \"Muted\""
        )
    }

    func test_unmute_re_enables_slider() {
        // Mute then unmute.
        muteCheckbox().click()
        _ = sliderElement().wait(timeout: 1.0) { !$0.enabled }
        muteCheckbox().click()

        let s = sliderElement()
        XCTAssertTrue(
            s.wait(timeout: 1.0) { $0.enabled },
            "slider should re-enable when mute is off"
        )
    }

    // ----------------------------------------------------------------
    // Pop-up selection
    // ----------------------------------------------------------------

    /// Pick a popup item by opening its menu (`kAXShowMenuAction`)
    /// and pressing the item by title. The popup's menu items
    /// only appear in the AX tree as children of an `AXMenu`
    /// child once the menu is open — `kAXValue` on the popup
    /// itself reports the current selection but isn't writable
    /// for selection changes.
    func test_pop_up_selection_updates_theme_label() {
        let popup = popupElement()

        // Open the menu. AppKit reveals the AXMenu under the popup.
        XCTAssertTrue(
            popup.perform(kAXShowMenuAction),
            "popup should accept kAXShowMenuAction"
        )

        // Find the "Dark" menu item once it appears.
        let dark = popup.waitForDescendant(timeout: 1.0) { el in
            el.role == kAXMenuItemRole as String
                && el.title == "Dark"
        }
        XCTAssertNotNil(dark, "Dark menu item should be visible")
        XCTAssertTrue(
            dark!.click(),
            "Dark menu item should accept kAXPressAction"
        )

        // The theme label updates reactively after selection.
        let label = session.window.waitForDescendant(timeout: 1.0) {
            $0.stringValue == "Selected theme: Dark"
        }
        XCTAssertNotNil(
            label,
            "theme label should reflect new popup selection (Dark)"
        )
    }

    // ----------------------------------------------------------------
    // Locator helpers
    // ----------------------------------------------------------------

    private func muteCheckbox() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXCheckBoxRole as String,
            title: "Mute audio"
        ) else {
            XCTFail("Mute audio checkbox missing")
            fatalError("unreachable")
        }
        return el
    }

    private func sliderElement() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXSliderRole as String
        ) else {
            XCTFail("slider missing")
            fatalError("unreachable")
        }
        return el
    }

    private func popupElement() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXPopUpButtonRole as String
        ) else {
            XCTFail("pop_up_button missing")
            fatalError("unreachable")
        }
        return el
    }
}
