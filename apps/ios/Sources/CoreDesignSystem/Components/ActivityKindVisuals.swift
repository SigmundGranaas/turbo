import SwiftUI
import CoreModel

/// The visual identity (SF Symbol + tint) for each activity kind, mirroring the
/// design's `IconsCard` (`ios/iosFoundations.jsx`) — the 18-activity system
/// mapped to the closest SF Symbol. Lives in the design system, keeping
/// `CoreModel.ActivityKindId` a pure type (as on Android).
public extension ActivityKindId {

    /// Localized display label. (English for now; `nb-NO` follows the Android
    /// localization roadmap.)
    var label: String {
        switch self {
        case .mountain: "Mountain"
        case .park: "Park"
        case .beach: "Beach"
        case .forest: "Forest"
        case .hiking: "Hiking"
        case .kayaking: "Kayaking"
        case .biking: "Biking"
        case .cabin: "Cabin"
        case .parking: "Parking"
        case .camping: "Camping"
        case .swimming: "Swimming"
        case .diving: "Diving"
        case .viewpoint: "Viewpoint"
        case .restaurant: "Restaurant"
        case .cafe: "Café"
        case .accommodation: "Accommodation"
        case .fishing: "Fishing"
        case .skiing: "Skiing"
        }
    }

    /// SF Symbol name approximating the codebase's Material icon.
    var symbolName: String {
        switch self {
        case .mountain: "mountain.2.fill"
        case .park: "leaf.fill"
        case .beach: "beach.umbrella.fill"
        case .forest: "tree.fill"
        case .hiking: "figure.hiking"
        case .kayaking: "sailboat.fill"
        case .biking: "bicycle"
        case .cabin: "house.fill"
        case .parking: "parkingsign"
        case .camping: "tent.fill"
        case .swimming: "figure.pool.swim"
        case .diving: "water.waves"
        case .viewpoint: "camera.fill"
        case .restaurant: "fork.knife"
        case .cafe: "cup.and.saucer.fill"
        case .accommodation: "bed.double.fill"
        case .fishing: "fish.fill"
        case .skiing: "figure.skiing.downhill"
        }
    }

    /// The system-color tint for this kind, mirroring the design's `IconsCard`.
    func tint(_ t: TurboColors) -> Color {
        switch self {
        case .mountain: t.green
        case .park: t.green
        case .beach: t.teal
        case .forest: t.green
        case .hiking: t.indigo
        case .kayaking: Color(hex: 0x0277BD)
        case .biking: t.red
        case .cabin: Color(hex: 0x6D4C41)
        case .parking: t.gray
        case .camping: t.orange
        case .swimming: t.teal
        case .diving: t.blue
        case .viewpoint: t.purple
        case .restaurant: t.orange
        case .cafe: Color(hex: 0x6D4C41)
        case .accommodation: t.pink
        case .fishing: t.blue
        case .skiing: t.teal
        }
    }
}
