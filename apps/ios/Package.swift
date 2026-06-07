// swift-tools-version: 6.0
import PackageDescription

// Turbo for iOS 26 — modular Swift package.
//
// Mirrors the native Android app's Gradle module graph (apps/android) one-to-one:
//   :core:model        -> CoreModel        (pure domain + geo, no UI)
//   :core:common       -> CoreCommon       (Outcome, dispatch helpers)
//   :core:designsystem -> CoreDesignSystem (iOS 26 Liquid Glass theme + components)
//   :core:map          -> CoreMap          (map + offline tile seams)
//   :feature:*         -> Feature*         (one MVVM screen package per feature)
//
// The thin Xcode app target (see project.yml) only assembles these products.
// Pure modules (CoreModel/CoreCommon/CoreMap) are host-testable via `swift test`;
// SwiftUI modules are built for the iOS simulator via `xcodebuild`.
let package = Package(
    name: "Turbo",
    platforms: [
        .iOS(.v18),
        .macOS(.v14),
    ],
    products: [
        .library(name: "CoreModel", targets: ["CoreModel"]),
        .library(name: "CoreCommon", targets: ["CoreCommon"]),
        .library(name: "CoreDesignSystem", targets: ["CoreDesignSystem"]),
        .library(name: "CoreMap", targets: ["CoreMap"]),
        .library(name: "FeatureOffline", targets: ["FeatureOffline"]),
        // Umbrella product the app links against — keeps project.yml stable as
        // new feature modules come online.
        .library(name: "TurboApp", targets: ["TurboApp"]),
    ],
    targets: [
        // ---- core ----
        .target(name: "CoreModel"),
        .target(name: "CoreCommon"),
        .target(
            name: "CoreDesignSystem",
            dependencies: ["CoreModel", "CoreCommon"]
        ),
        .target(
            name: "CoreMap",
            dependencies: ["CoreModel", "CoreCommon"]
        ),

        // ---- features ----
        .target(
            name: "FeatureOffline",
            dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreMap"]
        ),

        // ---- app assembly (composition root) ----
        .target(
            name: "TurboApp",
            dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreMap", "FeatureOffline"]
        ),

        // ---- tests ----
        .testTarget(name: "CoreModelTests", dependencies: ["CoreModel"]),
        .testTarget(name: "CoreMapTests", dependencies: ["CoreMap"]),
        .testTarget(name: "FeatureOfflineTests", dependencies: ["FeatureOffline"]),
    ]
)
