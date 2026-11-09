import XCTest

final class DiagnosticTests: XCTestCase {
    private func launch(_ entry: String, _ scenario: String = "default", _ theme: String = "light") -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["--entry=\(entry)", "--scenario=\(scenario)", "--theme=\(theme)", "--motion=reduce"]
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

    func testSharingHasNoSendAction() {
        let app = launch("composition.feedback", "sharing", "dark")
        XCTAssertTrue(app.staticTexts["Tác giả chỉ chia sẻ bài viết này."].waitForExistence(timeout: 15))
        XCTAssertFalse(app.buttons["Gửi nhận xét"].exists)
    }

    func testDisabledButtonHasNoDispatch() {
        let app = launch("action.button", "disabled")
        let button = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Lưu (")).firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 15))
        XCTAssertFalse(button.isEnabled)
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

}
