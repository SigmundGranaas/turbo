import SwiftUI
import CoreModel
import CoreDesignSystem

/// Native grouped settings — profile, units, appearance, sharing, maps & storage.
/// Mirrors `SettingsScreen` (design) / `feature.settings.SettingsScreen` (Android).
public struct SettingsScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: SettingsViewModel
    private let accountName: String?
    private let onOpenOffline: () -> Void

    public init(
        viewModel: SettingsViewModel,
        accountName: String? = nil,
        onOpenOffline: @escaping () -> Void = {}
    ) {
        _viewModel = State(initialValue: viewModel)
        self.accountName = accountName
        self.onOpenOffline = onOpenOffline
    }

    public var body: some View {
        // Read `settings` here in `body` so SwiftUI's Observation registers the
        // dependency and the rows re-render when the async repository update lands.
        let settings = viewModel.settings
        return List {
            Section {
                HStack(spacing: 14) {
                    if let accountName {
                        Monogram(initials: Self.initials(accountName), size: 58)
                        Text(accountName).font(.turboTitle3).foregroundStyle(t.label)
                    } else {
                        Image(systemName: "person.crop.circle.fill").font(.system(size: 52)).foregroundStyle(t.label3)
                        Text("Not signed in").font(.turboTitle3).foregroundStyle(t.label2)
                    }
                }
                .padding(.vertical, 4)
            }

            Section {
                Picker(selection: bind(settings.themeMode, viewModel.setThemeMode)) {
                    ForEach(ThemeMode.allCases, id: \.self) { Text($0.label).tag($0) }
                } label: {
                    rowLabel("Appearance", "circle.lefthalf.filled", t.indigo)
                }
                Toggle(isOn: bind(settings.metricUnits, viewModel.setMetricUnits)) {
                    rowLabel("Metric Units", "ruler", t.gray)
                }
                Toggle(isOn: bind(settings.compassOrientation, viewModel.setCompassOrientation)) {
                    rowLabel("Compass Orientation", "location.north.line", t.red)
                }
            }

            Section("Sharing & Privacy") {
                Toggle(isOn: bind(settings.shareLocation, viewModel.setShareLocation)) {
                    rowLabel("Share My Location", "person.2.fill", t.indigo)
                }
            }

            Section("Maps & Storage") {
                Button(action: onOpenOffline) {
                    rowLabel("Offline Maps", "arrow.down.circle.fill", t.green)
                }
                Toggle(isOn: bind(settings.avalancheAlerts, viewModel.setAvalancheAlerts)) {
                    rowLabel("Avalanche Alerts", "exclamationmark.triangle.fill", t.orange)
                }
            }
        }
        .navigationTitle("Settings")
        .task { viewModel.start() }
    }

    static func initials(_ name: String) -> String {
        let letters = name.split(separator: " ").prefix(2).compactMap { $0.first }.map(String.init).joined()
        return letters.isEmpty ? "?" : letters.uppercased()
    }

    private func rowLabel(_ title: String, _ symbol: String, _ color: Color) -> some View {
        HStack(spacing: 12) {
            Glyph(symbol: symbol, color: color, size: 29, cornerRadius: 7)
            Text(title).foregroundStyle(t.label)
        }
    }

    /// A binding whose value is the current snapshot and whose setter writes
    /// through the view model (which persists + re-emits).
    private func bind<Value>(_ value: Value, _ set: @escaping (Value) -> Void) -> Binding<Value> {
        Binding(get: { value }, set: { set($0) })
    }
}
