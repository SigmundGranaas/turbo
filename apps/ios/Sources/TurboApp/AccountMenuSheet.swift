import SwiftUI
import CoreDesignSystem

/// The account menu — tapped from the map avatar. Shows the account (or signed-out
/// state) and the entry points into Markers, Paths, Collections, Offline and
/// Settings. No fabricated identity, counts, or friend code.
struct AccountMenuSheet: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss

    let accountName: String?
    let accountEmail: String?
    /// The user's shareable friend code (signed in), else nil.
    var friendCode: String? = nil
    /// True when a sign-in flow is available (online build, signed out).
    let canSignIn: Bool
    let onSelect: (RootView.Route) -> Void
    let onAccount: () -> Void

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 0) {
                    header
                    if let friendCode { friendCodeRow(friendCode) }
                    group {
                        menuRow(.markers, "My Markers", "mappin.circle.fill", t.red)
                        divider
                        menuRow(.paths, "My Paths", "point.topleft.down.curvedto.point.bottomright.up", t.blue)
                        divider
                        menuRow(.collections, "Collections", "folder.fill", t.indigo)
                        divider
                        menuRow(.offline, "Offline Maps", "arrow.down.circle.fill", t.green)
                    }
                    group {
                        menuRow(.settings, "Settings", "gearshape.fill", t.gray)
                    }
                }
                .padding(.top, 8)
            }
            .background(t.grouped)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    @ViewBuilder
    private var header: some View {
        if canSignIn {
            Button { dismiss(); onAccount() } label: { headerContent }
                .buttonStyle(.plain)
                .accessibilityIdentifier("menu.account")
        } else {
            headerContent.accessibilityIdentifier("menu.account")
        }
    }

    private var headerContent: some View {
        HStack(spacing: 14) {
            if let initials = accountName.map(Self.initials) {
                Monogram(initials: initials, size: 60)
            } else {
                Image(systemName: "person.crop.circle.fill")
                    .font(.system(size: 56)).foregroundStyle(t.label3)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text(accountName ?? "Not signed in").font(.turboTitle2).foregroundStyle(t.label)
                if let accountEmail {
                    Text(accountEmail).font(.turboSubhead).foregroundStyle(t.label2)
                } else if canSignIn {
                    Text("Sign in to sync").font(.turboSubhead).foregroundStyle(t.blue)
                }
            }
            Spacer()
            if canSignIn {
                Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold)).foregroundStyle(t.label3)
            }
        }
        .padding(.horizontal, 18)
        .padding(.bottom, 16)
        .contentShape(Rectangle())
    }

    /// Real friend code (loaded from the sharing service) with a share action.
    private func friendCodeRow(_ code: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "qrcode").font(.system(size: 22)).foregroundStyle(t.label)
            VStack(alignment: .leading, spacing: 1) {
                Text("Friend code").font(.turboFootnote).foregroundStyle(t.label2)
                Text(code).font(.turboHeadline).foregroundStyle(t.label)
            }
            Spacer()
            ShareLink(item: code) {
                Text("Share").font(.turboSubhead.weight(.semibold)).foregroundStyle(.white)
                    .padding(.horizontal, 13).padding(.vertical, 6)
                    .background(t.blue, in: Capsule())
            }
        }
        .padding(14)
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .padding(.horizontal, 16)
        .padding(.bottom, 12)
    }

    private func group<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(spacing: 0) { content() }
            .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .padding(.horizontal, 16)
            .padding(.bottom, 12)
    }

    private var divider: some View {
        Rectangle().fill(t.separator).frame(height: 0.5).padding(.leading, 54)
    }

    private func menuRow(_ route: RootView.Route, _ title: String, _ symbol: String, _ color: Color) -> some View {
        Button {
            dismiss()
            onSelect(route)
        } label: {
            HStack(spacing: 12) {
                Glyph(symbol: symbol, color: color, size: 29, cornerRadius: 7)
                Text(title).font(.turboBody).foregroundStyle(t.label)
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold)).foregroundStyle(t.label3)
            }
            .padding(.horizontal, 14).padding(.vertical, 11)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("menu.\(route)")
    }

    /// First letters of the first two words of a display name.
    static func initials(_ name: String) -> String {
        let parts = name.split(separator: " ").prefix(2)
        let letters = parts.compactMap { $0.first }.map(String.init).joined()
        return letters.isEmpty ? "?" : letters.uppercased()
    }
}
