import Testing
import CoreModel
import CoreData
@testable import FeatureSettings

@Suite("SettingsViewModel")
@MainActor
struct SettingsViewModelTests {

    @Test("start() mirrors persisted settings and setters write through")
    func roundTrip() async {
        let repo = InMemorySettingsRepository()
        let vm = SettingsViewModel(repository: repo)
        vm.start()
        try? await Task.sleep(for: .milliseconds(120))
        #expect(vm.settings.metricUnits == true)

        vm.setMetricUnits(false)
        vm.setThemeMode(.dark)
        try? await Task.sleep(for: .milliseconds(120))
        #expect(vm.settings.metricUnits == false)
        #expect(vm.settings.themeMode == .dark)
        vm.stop()
    }
}
