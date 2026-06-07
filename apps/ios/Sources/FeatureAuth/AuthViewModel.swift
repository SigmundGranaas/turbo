import Foundation
import Observation
import CoreAuth

/// Drives sign-in and exposes the current ``AuthState``. Mirrors
/// `feature.auth.AuthViewModel` (Android).
@MainActor
@Observable
public final class AuthViewModel {
    public private(set) var state: AuthState = .signedOut
    public private(set) var isWorking = false

    private let repository: AuthRepository
    private var observation: Task<Void, Never>?

    public init(repository: AuthRepository) {
        self.repository = repository
    }

    public func start() {
        guard observation == nil else { return }
        observation = Task { [weak self, repository] in
            for await value in await repository.state() {
                self?.state = value
            }
        }
    }

    public func stop() { observation?.cancel(); observation = nil }

    public func signIn() {
        isWorking = true
        Task { [repository] in
            _ = await repository.signIn()
            self.isWorking = false
        }
    }

    public func signOut() {
        Task { [repository] in await repository.signOut() }
    }
}
