// End-to-end UI tests for `examples/counters_macos`.
//
// Exercises the `<For>` dynamic-children path: Add/Clear at the
// top, per-row +1/-1 buttons, total-label that sums every row's
// signal.

import AppDriver
import ApplicationServices
import XCTest

final class CountersUITests: XCTestCase {

    var session: AXSession!

    override func setUpWithError() throws {
        continueAfterFailure = false
        session = try AXSession.forExample("COUNTERS")
    }

    override func tearDown() {
        session?.terminate()
        super.tearDown()
    }

    func disabled_test_dump_tree_after_add() {
        addButton().click()
        addButton().click()
        usleep(500_000)
        print(session.window.dumpTree())
    }

    // ----------------------------------------------------------------
    // Initial state — no counter rows yet, just header buttons +
    // the total label
    // ----------------------------------------------------------------

    func test_window_present() {
        XCTAssertEqual(session.window.title, "Counters — dynamic")
    }

    func test_initial_no_rows() {
        XCTAssertEqual(
            counterRows().count, 0,
            "no rows should exist before Add is clicked"
        )
    }

    func test_initial_total_zero_zero_counters() {
        let label = totalLabel()
        XCTAssertEqual(
            label.stringValue,
            "Total: 0 from 0 counter(s)",
            "total label should show 0 / 0 initially"
        )
    }

    // ----------------------------------------------------------------
    // Add button creates a new row
    // ----------------------------------------------------------------

    func test_add_creates_row() {
        addButton().click()

        // The For-loop's mount_before path is async-ish (deferred
        // mount cascade + scheduled relayout). Give it a tick.
        let appeared = session.window.wait(timeout: 1.0) { win in
            !rowsIn(win).isEmpty
        }
        XCTAssertTrue(appeared, "row should appear after Add")
        XCTAssertEqual(counterRows().count, 1)

        // Total label updates to reflect 1 counter at 0.
        let totalUpdated = totalLabel().wait(timeout: 1.0) {
            $0.stringValue == "Total: 0 from 1 counter(s)"
        }
        XCTAssertTrue(totalUpdated)
    }

    func test_add_three_rows_three_present() {
        addButton().click()
        addButton().click()
        addButton().click()

        let three = session.window.wait(timeout: 1.0) {
            rowsIn($0).count == 3
        }
        XCTAssertTrue(three, "three rows should be present")
    }

    // ----------------------------------------------------------------
    // Per-row + and − buttons mutate that row's signal only
    // ----------------------------------------------------------------

    func test_increment_only_target_row() {
        addButton().click()
        addButton().click()
        _ = session.window.wait(timeout: 1.0) {
            rowsIn($0).count == 2
        }

        let rows = counterRows()
        // Click +1 on the second row twice.
        rows[1].plus.click()
        rows[1].plus.click()

        // Wait for that row's label to read "2".
        let updated = rows[1].value.wait(timeout: 1.0) {
            $0.stringValue == "2"
        }
        XCTAssertTrue(updated, "row[1] should read 2 after two +1s")

        // Row 0 still reads "0".
        XCTAssertEqual(
            rows[0].value.stringValue, "0",
            "row[0] should be untouched"
        )

        // Total reflects sum.
        XCTAssertTrue(totalLabel().wait(timeout: 1.0) {
            $0.stringValue == "Total: 2 from 2 counter(s)"
        })
    }

    func test_decrement_below_zero_negative() {
        addButton().click()
        _ = session.window.wait(timeout: 1.0) {
            rowsIn($0).count == 1
        }
        let row = counterRows()[0]
        row.minus.click()

        let updated = row.value.wait(timeout: 1.0) {
            $0.stringValue == "-1"
        }
        XCTAssertTrue(updated, "row should read -1 after a -1 click")
    }

    // ----------------------------------------------------------------
    // Clear removes all rows
    // ----------------------------------------------------------------

    func test_clear_removes_all_rows() {
        addButton().click()
        addButton().click()
        addButton().click()
        _ = session.window.wait(timeout: 1.0) {
            rowsIn($0).count == 3
        }

        clearButton().click()

        let cleared = session.window.wait(timeout: 1.0) {
            rowsIn($0).isEmpty
        }
        XCTAssertTrue(cleared, "all rows should be removed by Clear")

        XCTAssertEqual(
            totalLabel().stringValue,
            "Total: 0 from 0 counter(s)"
        )
    }

    // ----------------------------------------------------------------
    // Locator helpers
    // ----------------------------------------------------------------

    /// One counter row's three controls. AppKit doesn't surface
    /// our `<hstack>` as an AXGroup — it's a transparent NSView
    /// from AX's perspective — so rows appear as flat
    /// (-1, value, +1) triples interspersed with the header
    /// controls in `window.children`.
    struct Row {
        let minus: AXElement
        let value: AXElement
        let plus: AXElement
    }

    private func addButton() -> AXElement {
        button(titled: "Add")
    }

    private func clearButton() -> AXElement {
        button(titled: "Clear")
    }

    private func totalLabel() -> AXElement {
        guard let el = session.window.first(where: {
            ($0.stringValue ?? "").hasPrefix("Total:")
        }) else {
            XCTFail("Total label missing")
            fatalError("unreachable")
        }
        return el
    }

    private func counterRows() -> [Row] {
        rowsIn(session.window)
    }

    /// Scan `root.children` for adjacent (-1, label, +1) triples.
    /// Each triple is one row.
    private func rowsIn(_ root: AXElement) -> [Row] {
        let kids = root.children
        var rows: [Row] = []
        var i = 0
        while i + 2 < kids.count {
            let a = kids[i]
            let b = kids[i + 1]
            let c = kids[i + 2]
            if a.role == kAXButtonRole as String && a.title == "-1"
                && b.role == kAXStaticTextRole as String
                && c.role == kAXButtonRole as String && c.title == "+1"
            {
                rows.append(Row(minus: a, value: b, plus: c))
                i += 3
            } else {
                i += 1
            }
        }
        return rows
    }

    private func plusButton(in row: Row) -> AXElement { row.plus }
    private func minusButton(in row: Row) -> AXElement { row.minus }
    private func valueLabel(in row: Row) -> AXElement { row.value }

    private func button(titled t: String) -> AXElement {
        guard let el = session.window.firstChild(
            role: kAXButtonRole as String, title: t
        ) else {
            XCTFail("button titled \"\(t)\" missing")
            fatalError("unreachable")
        }
        return el
    }
}
