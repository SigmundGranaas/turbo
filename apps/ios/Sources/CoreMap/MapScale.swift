import Foundation

/// Computes the map scale bar — the largest "round" ground distance that fits a
/// pixel budget, plus its bar width. Pure, so the label/width math is testable.
public enum MapScale {
    private static let niceMeters: [Double] = [
        10, 20, 50, 100, 200, 500, 1_000, 2_000, 5_000,
        10_000, 20_000, 50_000, 100_000, 200_000, 500_000,
    ]

    public static func bar(metersPerPoint: Double, maxWidthPoints: Double) -> (label: String, widthPoints: Double) {
        guard metersPerPoint > 0, maxWidthPoints > 0 else { return (label: "—", widthPoints: 0) }
        let budgetMeters = metersPerPoint * maxWidthPoints
        let meters = niceMeters.last { $0 <= budgetMeters } ?? niceMeters.first!
        let width = meters / metersPerPoint
        let label = meters >= 1_000 ? "\(Int(meters / 1_000)) km" : "\(Int(meters)) m"
        return (label: label, widthPoints: width)
    }
}
