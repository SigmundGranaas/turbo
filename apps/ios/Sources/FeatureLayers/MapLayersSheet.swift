import SwiftUI
import CoreModel
import CoreDesignSystem

/// The base-map picker + overlay toggles, presented as a sheet over the map.
/// Mirrors `MapLayers` (design) / `feature.layers.MapLayersSheet` (Android).
public struct MapLayersSheet: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss

    @Binding var baseLayer: BaseLayer
    @Binding var overlays: Set<OverlayId>

    public init(baseLayer: Binding<BaseLayer>, overlays: Binding<Set<OverlayId>>) {
        _baseLayer = baseLayer
        _overlays = overlays
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    baseRow
                    Text("Overlays")
                        .font(.turboFootnote)
                        .foregroundStyle(t.label2)
                        .textCase(.uppercase)
                        .padding(.horizontal, 16)
                        .padding(.top, 18)
                        .padding(.bottom, 8)
                    overlayList
                }
                .padding(.vertical, 8)
            }
            .background(t.grouped)
            .navigationTitle("Maps")
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private var baseRow: some View {
        HStack(spacing: 12) {
            ForEach(BaseLayer.allCases, id: \.self) { layer in
                BaseTile(layer: layer, selected: layer == baseLayer) {
                    baseLayer = layer
                }
            }
        }
        .padding(.horizontal, 16)
    }

    private var overlayList: some View {
        VStack(spacing: 0) {
            ForEach(Array(OverlayId.allCases.enumerated()), id: \.element) { index, overlay in
                Toggle(isOn: binding(for: overlay)) {
                    HStack(spacing: 12) {
                        Glyph(symbol: overlay.symbol, color: overlay.tint(t), size: 29, cornerRadius: 7)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(overlay.title).font(.turboBody).foregroundStyle(t.label)
                            Text(overlay.subtitle).font(.turboFootnote).foregroundStyle(t.label2)
                        }
                    }
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                if index < OverlayId.allCases.count - 1 {
                    Rectangle().fill(t.separator).frame(height: 0.5).padding(.leading, 54)
                }
            }
        }
        .background(t.groupedCard)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .padding(.horizontal, 16)
    }

    private func binding(for overlay: OverlayId) -> Binding<Bool> {
        Binding(
            get: { overlays.contains(overlay) },
            set: { isOn in
                if isOn { overlays.insert(overlay) } else { overlays.remove(overlay) }
            }
        )
    }
}

/// A base-map thumbnail tile with a selected ring. (A real tile preview lands
/// with the map snapshotter.)
private struct BaseTile: View {
    @Environment(\.turbo) private var t
    let layer: BaseLayer
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 7) {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(swatch)
                    .frame(height: 86)
                    .overlay(
                        Image(systemName: symbol)
                            .font(.system(size: 26, weight: .regular))
                            .foregroundStyle(t.label2)
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .stroke(selected ? t.blue : t.separator, lineWidth: selected ? 3 : 1)
                    )
                Text(layer.title)
                    .font(.turboFootnote)
                    .fontWeight(selected ? .semibold : .regular)
                    .foregroundStyle(selected ? t.blue : t.label)
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(layer.title)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private var symbol: String {
        switch layer {
        case .norgeskart: "mountain.2"
        case .osm: "map"
        case .satellite: "globe.europe.africa.fill"
        }
    }

    private var swatch: Color {
        switch layer {
        case .norgeskart: Color(hex: 0xDAD6C2)
        case .osm: t.dark ? Color(hex: 0x2A2A2C) : Color(hex: 0xE9EEF0)
        case .satellite: Color(hex: 0x29362A)
        }
    }
}

private extension OverlayId {
    var symbol: String {
        switch self {
        case .trails: "point.topleft.down.curvedto.point.bottomright.up"
        case .waves: "water.waves"
        case .wind: "wind"
        case .avalanche: "exclamationmark.triangle.fill"
        }
    }
    func tint(_ t: TurboColors) -> Color {
        switch self {
        case .trails: t.red
        case .waves: t.teal
        case .wind: t.indigo
        case .avalanche: t.orange
        }
    }
}
