import SwiftUI
import CoreModel
import CoreDesignSystem

/// Varsom-style avalanche danger detail — the 1–5 level, its label and headline.
public struct AvalancheDetailScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: AvalancheViewModel

    public init(viewModel: AvalancheViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        ScrollView {
            if let info = viewModel.info {
                VStack(alignment: .leading, spacing: 20) {
                    HStack(spacing: 16) {
                        ZStack {
                            RoundedRectangle(cornerRadius: 16, style: .continuous).fill(color(info.level))
                            Text("\(info.level)").font(.system(size: 40, weight: .bold)).foregroundStyle(.white)
                        }
                        .frame(width: 84, height: 84)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(info.label).font(.turboTitle).foregroundStyle(t.label)
                            Text(info.region).font(.turboSubhead).foregroundStyle(t.label2)
                        }
                    }
                    Text(info.headline)
                        .font(.turboBody).foregroundStyle(t.label)
                        .padding(14)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                    Text("Source: Varsom / NVE")
                        .font(.turboFootnote).foregroundStyle(t.label2)
                }
                .padding(16)
            } else if viewModel.loaded {
                ContentUnavailableView(
                    "No Warning",
                    systemImage: "checkmark.shield",
                    description: Text("No avalanche warning is issued for this area.")
                )
                .padding(.top, 60)
            } else {
                ProgressView().padding(40)
            }
        }
        .background(t.grouped)
        .navigationTitle("Avalanche Danger")
        .toolbarTitleDisplayMode(.inline)
        .task { await viewModel.load() }
    }

    private func color(_ level: Int) -> Color {
        switch level {
        case 1: t.green
        case 2: t.yellow
        case 3: t.orange
        case 4: t.red
        default: Color(hex: 0x7B1FA2)
        }
    }
}
