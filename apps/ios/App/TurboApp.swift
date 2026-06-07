import SwiftUI
import TurboApp

/// The iOS app entry point. Keeps the executable target paper-thin: it owns the
/// ``AppContainer`` (composition root) and hands it to `RootView`. All real code
/// lives in the Swift package modules.
@main
struct TurboiOSApp: App {
    @State private var container = AppContainer.resolve()

    var body: some Scene {
        WindowGroup {
            RootView(container: container)
        }
    }
}
