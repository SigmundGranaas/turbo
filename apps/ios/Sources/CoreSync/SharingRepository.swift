import Foundation
import CoreCommon

/// A redeemed share link: which resource the user gained access to, and the role.
/// Mirrors `core.sync.LinkRedemption`.
public struct LinkRedemption: Equatable, Sendable {
    public let resourceId: String
    public let resourceType: String
    public let role: String
    public init(resourceId: String, resourceType: String, role: String) {
        self.resourceId = resourceId
        self.resourceType = resourceType
        self.role = role
    }
}

/// Authenticated access to the backend sharing service — friend code + share
/// links. Only usable when signed in (token-authed); the app hides sharing
/// otherwise. Mirrors `core.sync.SharingRepository`.
public protocol SharingRepository: Sendable {
    /// The user's shareable friend code (e.g. "turbo-AB12CD").
    func friendCode() async -> Outcome<String>
    /// Create a share link for a resource; returns a shareable URL.
    func createLink(resourceId: String, role: String) async -> Outcome<URL>
    /// Redeem a share-link token, granting the current user access.
    func redeemLink(token: String) async -> Outcome<LinkRedemption>
}

public extension SharingRepository {
    func createLink(resourceId: String) async -> Outcome<URL> {
        await createLink(resourceId: resourceId, role: "viewer")
    }
}

/// HTTP sharing client against the API (token-authed). Network path — the pure
/// pieces (`friendCode` formatting, link URL, redemption decode) are static + tested.
public struct HttpSharingRepository: SharingRepository {
    static let friendCodePrefix = "turbo-"

    private let apiBaseURL: URL
    private let webBaseURL: URL
    private let token: @Sendable () async -> String?
    private let session: URLSession

    public init(apiBaseURL: URL, webBaseURL: URL, token: @escaping @Sendable () async -> String?, session: URLSession = .shared) {
        self.apiBaseURL = apiBaseURL
        self.webBaseURL = webBaseURL
        self.token = token
        self.session = session
    }

    public func friendCode() async -> Outcome<String> {
        await request("sharing/me/profile", method: "GET") { data in
            guard let dto = try? JSONDecoder().decode(ProfileDTO.self, from: data) else { return nil }
            return Self.formatFriendCode(dto.friendCode)
        }
    }

    public func createLink(resourceId: String, role: String) async -> Outcome<URL> {
        let body = try? JSONSerialization.data(withJSONObject: ["resourceId": resourceId, "role": role])
        return await request("sharing/grants/links", method: "POST", body: body) { data in
            guard let dto = try? JSONDecoder().decode(LinkGrantDTO.self, from: data) else { return nil }
            return Self.linkURL(webBase: webBaseURL, token: dto.linkToken)
        }
    }

    public func redeemLink(token: String) async -> Outcome<LinkRedemption> {
        await request("sharing/grants/links/\(token)/redeem", method: "POST") { data in
            Self.decodeRedemption(data)
        }
    }

    // MARK: - Pure helpers (tested)

    static func formatFriendCode(_ code: String) -> String { "\(friendCodePrefix)\(code)" }

    static func linkURL(webBase: URL, token: String) -> URL {
        webBase.appendingPathComponent("link").appendingPathComponent(token)
    }

    static func decodeRedemption(_ data: Data) -> LinkRedemption? {
        guard let dto = try? JSONDecoder().decode(RedemptionDTO.self, from: data) else { return nil }
        return LinkRedemption(resourceId: dto.resourceId, resourceType: dto.resourceType, role: dto.role)
    }

    // MARK: - HTTP

    private func request<T>(_ path: String, method: String, body: Data? = nil,
                            decode: @Sendable (Data) -> T?) async -> Outcome<T> {
        var req = URLRequest(url: apiBaseURL.appendingPathComponent(path))
        req.httpMethod = method
        if let body { req.httpBody = body; req.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        if let token = await token() { req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        do {
            let (data, response) = try await session.data(for: req)
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
                  let value = decode(data) else {
                return .failure(NSError(domain: "Turbo.Sharing", code: 1))
            }
            return .success(value)
        } catch {
            return .failure(error)
        }
    }

    private struct ProfileDTO: Decodable { let friendCode: String }
    private struct LinkGrantDTO: Decodable { let linkToken: String }
    private struct RedemptionDTO: Decodable { let resourceId: String; let resourceType: String; let role: String }
}

/// In-memory sharing double for tests / hermetic UI runs.
public actor InMemorySharingRepository: SharingRepository {
    private let code: String
    private var links: [String: String] = [:]   // token → resourceId

    public init(code: String = "turbo-TEST01") { self.code = code }

    public func friendCode() -> Outcome<String> { .success(code) }

    public func createLink(resourceId: String, role: String) -> Outcome<URL> {
        let token = UUID().uuidString.prefix(8).lowercased()
        links[String(token)] = resourceId
        return .success(URL(string: "https://kart.sandring.no/link/\(token)")!)
    }

    public func redeemLink(token: String) -> Outcome<LinkRedemption> {
        guard let resourceId = links[token] else { return .failure(NSError(domain: "Turbo.Sharing", code: 404)) }
        return .success(LinkRedemption(resourceId: resourceId, resourceType: "marker", role: "viewer"))
    }
}
