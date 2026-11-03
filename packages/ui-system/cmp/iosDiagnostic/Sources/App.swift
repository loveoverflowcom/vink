import UIKit
import VinkDiagnostics

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    var session: NativeSession?
    var motionObserver: NSObjectProtocol?

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        let args = ProcessInfo.processInfo.arguments.filter { $0.hasPrefix("--") }
        let session = NativeSession(args: args)
        self.session = session
        let window = UIWindow(frame: UIScreen.main.bounds)
        window.rootViewController = session.controller()
        window.makeKeyAndVisible()
        self.window = window
        motionObserver = NotificationCenter.default.addObserver(forName: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            self?.activity(application.applicationState == .active)
        }
        activity(true)
        return true
    }

    private func activity(_ active: Bool) {
        session?.setActivity(active: active, reducedMotion: UIAccessibility.isReduceMotionEnabled)
    }
    func applicationDidBecomeActive(_ application: UIApplication) { activity(true) }
    func applicationWillResignActive(_ application: UIApplication) { activity(false) }
}
