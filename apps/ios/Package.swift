// swift-tools-version: 6.0
import PackageDescription

// Turbo for iOS 26 — modular Swift package.
//
// Mirrors the native Android app's Gradle module graph (apps/android) one-to-one:
//   :core:model        -> CoreModel        (pure domain + geo, no UI)
//   :core:common       -> CoreCommon       (Outcome, ReactiveStore)
//   :core:designsystem -> CoreDesignSystem (iOS 26 Liquid Glass theme + components)
//   :core:data         -> CoreData         (repository seams + in-memory impls)
//   :core:auth         -> CoreAuth         (auth state + sign-in seam)
//   :core:map          -> CoreMap          (map rendering + offline tile seams)
//   :feature:*         -> Feature*         (one MVVM screen package per feature)
//
// The thin Xcode app target (see project.yml) only assembles these products.
// Pure modules are host-testable via `swift test`; SwiftUI modules are built for
// the iOS simulator via `xcodebuild`.
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
        .library(name: "CoreData", targets: ["CoreData"]),
        .library(name: "CoreAuth", targets: ["CoreAuth"]),
        .library(name: "CoreSync", targets: ["CoreSync"]),
        .library(name: "CoreMap", targets: ["CoreMap"]),
        .library(name: "FeatureMap", targets: ["FeatureMap"]),
        .library(name: "FeatureLayers", targets: ["FeatureLayers"]),
        .library(name: "FeatureSearch", targets: ["FeatureSearch"]),
        .library(name: "FeatureSettings", targets: ["FeatureSettings"]),
        .library(name: "FeatureRecording", targets: ["FeatureRecording"]),
        .library(name: "FeatureAuth", targets: ["FeatureAuth"]),
        .library(name: "FeatureCollections", targets: ["FeatureCollections"]),
        .library(name: "FeatureOffline", targets: ["FeatureOffline"]),
        .library(name: "TurboApp", targets: ["TurboApp"]),
    ],
    targets: [
        // ---- core ----
        .target(name: "CoreModel"),
        .target(name: "CoreCommon"),
        .target(name: "CoreDesignSystem", dependencies: ["CoreModel", "CoreCommon"]),
        .target(name: "CoreData", dependencies: ["CoreModel", "CoreCommon"]),
        .target(name: "CoreAuth", dependencies: ["CoreCommon"]),
        .target(name: "CoreSync", dependencies: ["CoreModel", "CoreCommon", "CoreData", "CoreAuth"]),
        .target(name: "CoreMap", dependencies: ["CoreModel", "CoreCommon"]),

        // ---- features ----
        .target(name: "FeatureMap", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreData", "CoreMap"]),
        .target(name: "FeatureLayers", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem"]),
        .target(name: "FeatureSearch", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreData"]),
        .target(name: "FeatureSettings", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreData"]),
        .target(name: "FeatureRecording", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreData"]),
        .target(name: "FeatureAuth", dependencies: ["CoreCommon", "CoreDesignSystem", "CoreAuth"]),
        .target(name: "FeatureCollections", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreData"]),
        .target(name: "FeatureOffline", dependencies: ["CoreModel", "CoreCommon", "CoreDesignSystem", "CoreMap"]),

        // ---- app assembly (composition root) ----
        .target(
            name: "TurboApp",
            dependencies: [
                "CoreModel", "CoreCommon", "CoreDesignSystem", "CoreData", "CoreAuth", "CoreSync", "CoreMap",
                "FeatureMap", "FeatureLayers", "FeatureSearch", "FeatureSettings",
                "FeatureRecording", "FeatureAuth", "FeatureCollections", "FeatureOffline",
            ]
        ),

        // ---- tests ----
        .testTarget(name: "CoreModelTests", dependencies: ["CoreModel"]),
        .testTarget(name: "CoreCommonTests", dependencies: ["CoreCommon"]),
        .testTarget(name: "CoreDataTests", dependencies: ["CoreData"]),
        .testTarget(name: "CoreAuthTests", dependencies: ["CoreAuth"]),
        .testTarget(name: "CoreSyncTests", dependencies: ["CoreSync", "CoreData", "CoreAuth"]),
        .testTarget(name: "CoreMapTests", dependencies: ["CoreMap"]),
        .testTarget(name: "FeatureMapTests", dependencies: ["FeatureMap"]),
        .testTarget(name: "FeatureRecordingTests", dependencies: ["FeatureRecording", "CoreModel", "CoreData"]),
        .testTarget(name: "FeatureSettingsTests", dependencies: ["FeatureSettings"]),
        .testTarget(name: "FeatureCollectionsTests", dependencies: ["FeatureCollections", "CoreModel", "CoreData"]),
        .testTarget(name: "FeatureSearchTests", dependencies: ["FeatureSearch"]),
        .testTarget(name: "FeatureOfflineTests", dependencies: ["FeatureOffline"]),
    ]
)
