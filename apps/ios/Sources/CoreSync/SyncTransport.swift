import Foundation
import CoreCommon

/// The server side of sync for one entity type — pull the remote set, push local
/// changes. Generic over the wire ``Payload``. Mirrors `core.sync.SyncHttp`.
public protocol SyncTransport<Payload>: Sendable {
    associatedtype Payload: Codable & Sendable & Equatable
    func pull() async throws -> [SyncRecord<Payload>]
    func push(_ records: [SyncRecord<Payload>]) async throws
}

/// An in-memory "server" replica — applies pushes last-write-wins, serves pulls.
/// Used by tests and as an offline stand-in.
public actor InMemorySyncTransport<Payload: Codable & Sendable & Equatable>: SyncTransport {
    private var store: [String: SyncRecord<Payload>]

    public init(seed: [SyncRecord<Payload>] = []) {
        store = Dictionary(seed.map { ($0.id, $0) }, uniquingKeysWith: { a, b in a.updatedAt >= b.updatedAt ? a : b })
    }

    public func pull() -> [SyncRecord<Payload>] { Array(store.values) }

    public func push(_ records: [SyncRecord<Payload>]) {
        for record in records {
            if let existing = store[record.id], existing.updatedAt >= record.updatedAt { continue }
            store[record.id] = record
        }
    }
}

/// HTTP transport against the Turbo API for one entity collection (`/markers`,
/// `/paths`, …). Bearer-token auth supplied per call. (Network path — exercised
/// at runtime, not in unit tests.)
public struct HttpSyncTransport<Payload: Codable & Sendable & Equatable>: SyncTransport {
    private let endpoint: URL
    private let token: @Sendable () async -> String?
    private let session: URLSession

    public init(endpoint: URL, token: @escaping @Sendable () async -> String?, session: URLSession = .shared) {
        self.endpoint = endpoint
        self.token = token
        self.session = session
    }

    public func pull() async throws -> [SyncRecord<Payload>] {
        var request = URLRequest(url: endpoint)
        await authorize(&request)
        let (data, _) = try await session.data(for: request)
        return try JSONDecoder().decode([SyncRecord<Payload>].self, from: data)
    }

    public func push(_ records: [SyncRecord<Payload>]) async throws {
        guard !records.isEmpty else { return }
        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        await authorize(&request)
        request.httpBody = try JSONEncoder().encode(records)
        _ = try await session.data(for: request)
    }

    private func authorize(_ request: inout URLRequest) async {
        if let token = await token() {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
    }
}
