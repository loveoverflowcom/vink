import XCTest

final class DiagnosticTests: XCTestCase {
    private func waitForStationary(_ element: XCUIElement) {
        // XCTest's app-idle check does not wait for Compose scroll deceleration.
        // Resolve the hit point only after consecutive snapshots agree on its frame.
        var previous: CGRect?
        var stableSamples = 0
        let stationary = NSPredicate { _, _ in
            guard element.exists, element.isHittable else { stableSamples = 0; return false }
            let frame = element.frame
            stableSamples = previous == frame ? stableSamples + 1 : 0
            previous = frame
            return stableSamples >= 2
        }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: stationary, object: element)], timeout: 5), .completed)
    }

    private func launch(_ entry: String, _ scenario: String = "default", _ theme: String = "light", _ scale: String = "100") -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--entry=\(entry)", "--scenario=\(scenario)", "--theme=\(theme)", "--motion=reduce", "--scale=\(scale)"]
        app.launch()
        return app
    }

    func testControlledInputThemeAndKeyboard() {
        let app = launch("field.text-field")
        let field = app.textViews.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        field.tap()
        field.typeText("Vink 2026")
        app.buttons["Đổi sáng/tối"].tap()
        XCTAssertEqual(field.label, "Vink 2026")
        XCTAssertTrue(field.isHittable)
    }

    func testSharingKeepsComposerAndExplicitSubmitIntent() {
        // D2T-03: sharing is an intent label; only commentsClosed disables submitting.
        let app = launch("composition.feedback", "sharing", "dark")
        XCTAssertTrue(app.staticTexts["Chỉ chia sẻ"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.textViews.firstMatch.exists)
        let submit = app.buttons["Gửi nhận xét"]
        for _ in 0..<5 {
            if submit.isHittable { break }
            app.swipeUp()
        }
        XCTAssertTrue(submit.isEnabled)
        XCTAssertTrue(submit.isHittable)
        waitForStationary(submit)
        submit.tap()
        XCTAssertTrue(app.staticTexts["Mẫu · Intent gửi nhận xét, chưa gửi hoặc lưu."].waitForExistence(timeout: 5))
    }

    func testDisabledButtonHasNoDispatch() {
        let app = launch("action.button", "disabled")
        let button = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Lưu (")).firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 15))
        XCTAssertFalse(button.isEnabled)
    }

    func testInteractionSheetKeepsTextThroughKeyboardAndClose() {
        let app = launch("composition.interaction", "sheet")
        let trigger = app.buttons["Mở bảng nhập [Mẫu]"]
        XCTAssertTrue(trigger.waitForExistence(timeout: 15))
        if !trigger.isHittable { app.swipeUp() }
        trigger.tap()
        let area = app.textViews.firstMatch
        XCTAssertTrue(area.waitForExistence(timeout: 10))
        area.tap()
        area.typeText("Vink")
        let close = app.buttons["Giữ chữ và đóng"]
        // Swipe inside the sheet above the IME, not on the keyboard at the bottom of the app.
        let panel = app.descendants(matching: .any).matching(identifier: "interaction-panel").firstMatch
        for _ in 0..<4 {
            if close.isHittable { break }
            let start = panel.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.8))
            let end = panel.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.25))
            start.press(forDuration: 0.05, thenDragTo: end)
        }
        waitForStationary(close)
        let capture = XCTAttachment(screenshot: app.screenshot())
        capture.name = "Interaction sheet – keyboard and reachable close"
        capture.lifetime = .keepAlways
        add(capture)
        XCTAssertTrue(close.isHittable)
        close.tap()
        XCTAssertTrue(close.waitForNonExistence(timeout: 5))
        XCTAssertTrue(app.textViews.matching(NSPredicate(format: "label CONTAINS %@", "Vink")).firstMatch.exists)
    }

    func testWebpSpecimenShowsDecodedImageAndFailedFallback() {
        let alt = "[Mẫu] Ảnh nền lavender"
        let ready = launch("identity.image-frame")
        let image = ready.descendants(matching: .any).matching(NSPredicate(format: "label == %@", alt)).firstMatch
        XCTAssertTrue(image.waitForExistence(timeout: 15))
        let capture = XCTAttachment(screenshot: ready.screenshot())
        capture.name = "WebP ready – shared landscape and diagnostic detail"
        capture.lifetime = .keepAlways
        add(capture)

        let errors = launch("identity.image-frame", "image-error", "dark")
        let failure = errors.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@", "[Mẫu] Bìa tập thơ — Không tải được ảnh")
        )
        let bothFailed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "count == 2"), object: failure)
        XCTAssertEqual(XCTWaiter.wait(for: [bothFailed], timeout: 15), .completed)
        XCTAssertFalse(errors.descendants(matching: .any).matching(NSPredicate(format: "label == %@", alt)).firstMatch.exists)
    }

    // Alpha pixels are verified separately by EmptyArtworkTest on iosSimulatorArm64.
    // This XCTest checks the actual host's accessible copy, scroll and sample callback.
    func testArtworkEmptyStatesKeepRecoveryAtLargeText() {
        for entry in ["screen.explore", "screen.desk"] {
            for scenario in ["empty", "no-results"] {
                for theme in ["light", "dark"] {
                    let app = launch(entry, scenario, theme, "200")
                    let label = scenario == "no-results" ? "Xóa tìm kiếm" :
                        (entry == "screen.desk" ? "Viết bản thảo đầu tiên" : "Bắt đầu viết")
                    let action = app.buttons[label]
                    XCTAssertTrue(action.waitForExistence(timeout: 15))
                    for _ in 0..<6 {
                        if action.isHittable { break }
                        app.swipeUp()
                    }
                    waitForStationary(action)
                    let capture = XCTAttachment(screenshot: app.screenshot())
                    capture.name = "\(entry)-\(scenario)-\(theme)-text200"
                    capture.lifetime = .keepAlways
                    add(capture)
                    action.tap()
                    let result = scenario == "no-results" ? "Mẫu · Đã xóa tìm kiếm; không gọi dịch vụ." : "Mẫu · Đích #screen.editor/dirty"
                    XCTAssertTrue(app.staticTexts[result].waitForExistence(timeout: 5))
                    app.terminate()
                }
            }
        }
    }

}
