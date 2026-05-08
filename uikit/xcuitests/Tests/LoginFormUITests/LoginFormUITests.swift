// End-to-end UI tests for `examples/login_form_macos`.
//
// Drives the .app via the Accessibility framework (see `AppDriver`).
// Run with `xcuitests/run_tests.sh` — that script builds the .app,
// sets `LEPTOS_MAC_APP_PATH`, and invokes `swift test`.

import AppDriver
import ApplicationServices
import XCTest

final class LoginFormUITests: XCTestCase {

    var session: AXSession!

    override func setUpWithError() throws {
        continueAfterFailure = false
        session = try AXSession.forExample("LOGIN_FORM")
    }

    override func tearDown() {
        session?.terminate()
        super.tearDown()
    }

    // ----------------------------------------------------------------
    // Launch / structure
    // ----------------------------------------------------------------

    func test_window_present_with_title() {
        let win = session.window
        XCTAssertEqual(
            win.role, kAXWindowRole as String,
            "session.window should be an AXWindow"
        )
        XCTAssertEqual(win.title, "Login")
    }

    func test_initial_controls_present() {
        let win = session.window

        // Both text fields report `role=AXTextField`. The username
        // has no subrole; the password (NSSecureTextField) has
        // `subrole=AXSecureTextField`. We rely on the subrole split
        // to distinguish them.
        XCTAssertEqual(
            win.allChildren(
                role: kAXTextFieldRole as String,
                subrole: nil
            ).count,
            1,
            "expected one plain text field (username)"
        )
        XCTAssertEqual(
            win.allChildren(
                role: kAXTextFieldRole as String,
                subrole: "AXSecureTextField"
            ).count,
            1,
            "expected one secure text field (password)"
        )

        XCTAssertNotNil(
            win.firstChild(
                role: kAXCheckBoxRole as String,
                title: "Remember me on this device"
            ),
            "checkbox should be present"
        )
        XCTAssertNotNil(
            win.firstChild(
                role: kAXButtonRole as String,
                title: "Sign in"
            ),
            "Sign in button should be present"
        )
    }

    // ----------------------------------------------------------------
    // Enabled-state gating (the `enabled=move || can_submit.get()`
    // closure)
    // ----------------------------------------------------------------

    func test_sign_in_disabled_initially() {
        let signIn = signInButton()
        XCTAssertFalse(
            signIn.enabled,
            "Sign in should start disabled (empty username + password)"
        )
    }

    func test_sign_in_enables_with_valid_input() {
        usernameField().typeText("alice")
        passwordField().typeText("longenough")

        let ok = signInButton().wait(timeout: 1.0) { $0.enabled }
        XCTAssertTrue(
            ok,
            "Sign in should enable once username + 8+ char password are set"
        )
    }

    func test_sign_in_stays_disabled_for_short_password() {
        usernameField().typeText("alice")
        passwordField().typeText("short") // 5 chars

        // Give the reactive scheduler a tick to react before
        // asserting "still disabled" — otherwise we'd race.
        let elapsed = signInButton().wait(timeout: 0.5) { $0.enabled }
        XCTAssertFalse(
            elapsed,
            "Sign in should remain disabled for password under 8 chars"
        )
    }

    // ----------------------------------------------------------------
    // Checkbox toggle
    // ----------------------------------------------------------------

    func test_remember_checkbox_toggles() {
        let checkbox = rememberCheckbox()

        XCTAssertEqual(
            checkbox.numberValue?.intValue, 0,
            "checkbox should start unchecked"
        )
        checkbox.click()
        XCTAssertTrue(
            checkbox.wait(timeout: 0.5) {
                $0.numberValue?.intValue == 1
            },
            "checkbox should be checked after click"
        )
        checkbox.click()
        XCTAssertTrue(
            checkbox.wait(timeout: 0.5) {
                $0.numberValue?.intValue == 0
            },
            "checkbox should be unchecked after second click"
        )
    }

    // ----------------------------------------------------------------
    // Submit flow — full round-trip through reactive state
    // ----------------------------------------------------------------

    func test_submit_populates_status_label() {
        usernameField().typeText("alice")
        passwordField().typeText("longenough")
        rememberCheckbox().click()

        // Wait for Sign in to enable, then click.
        let signIn = signInButton()
        XCTAssertTrue(
            signIn.wait(timeout: 1.0) { $0.enabled },
            "Sign in should be enabled before submit"
        )
        signIn.click()

        // The example sets `status` to:
        //   "Signed in as alice (remember=true)"
        // The status label is one of the static-text descendants.
        let status = session.window.waitForDescendant(timeout: 2.0) {
            el in
            guard el.role == kAXStaticTextRole as String else {
                return false
            }
            let text = el.stringValue ?? el.title ?? ""
            return text.contains("Signed in as alice")
                && text.contains("remember=true")
        }
        XCTAssertNotNil(
            status,
            "status label should reflect submitted values"
        )
    }

    // ----------------------------------------------------------------
    // Locator helpers
    // ----------------------------------------------------------------
    //
    // Kept inline rather than on AXSession because they're specific
    // to this example's layout.

    private func usernameField() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXTextFieldRole as String,
            subrole: nil
        ) else {
            XCTFail("username text field missing")
            fatalError("unreachable")
        }
        return el
    }

    private func passwordField() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXTextFieldRole as String,
            subrole: "AXSecureTextField"
        ) else {
            XCTFail("password (secure) text field missing")
            fatalError("unreachable")
        }
        return el
    }

    private func rememberCheckbox() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXCheckBoxRole as String,
            title: "Remember me on this device"
        ) else {
            XCTFail("Remember-me checkbox missing")
            fatalError("unreachable")
        }
        return el
    }

    private func signInButton() -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXButtonRole as String,
            title: "Sign in"
        ) else {
            XCTFail("Sign in button missing")
            fatalError("unreachable")
        }
        return el
    }
}
