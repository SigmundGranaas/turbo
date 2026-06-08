import SwiftUI
import CoreDesignSystem

/// The account menu — tapped from the map avatar. Profile header, friend code,
/// and the entry points into Paths, Collections, Offline Maps and Settings.
/// Mirrors `NavMenu` in the design bundle.
struct AccountMenuSheet: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss

    let accountName: String
    let accountEmail: String?
    let onSelect: (RootView.Route) -> Void
    let onAccount: () -> Void

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 0) {
                    header
                    friendCode
                    group {
                        menuRow(.markers, "My Markers", "mappin.circle.fill", t.red, "Saved places")
                        divider
                        menuRow(.paths, "My Paths", "point.topleft.down.curvedto.point.bottomright.up", t.blue, "12 recorded")
                        divider
                        menuRow(.collections, "Collections", "folder.fill", t.indigo, "3 folders")
                        divider
                        menuRow(.offline, "Offline Maps", "arrow.down.circle.fill", t.green, "2 regions · 1.4 GB")
                    }
                    group {
                        menuRow(.settings, "Settings", "gearshape.fill", t.gray, nil)
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

    private var header: some View {
        Button {
            dismiss()
            onAccount()
        } label: {
            HStack(spacing: 14) {
                Monogram(initials: "SG", size: 60)
                VStack(alignment: .leading, spacing: 4) {
                    Text(accountName).font(.turboTitle2).foregroundStyle(t.label)
                    if let accountEmail {
                        Text(accountEmail).font(.turboSubhead).foregroundStyle(t.label2)
                        HStack(spacing: 5) {
                            Image(systemName: "sparkles").font(.system(size: 12, weight: .semibold))
                            Text("Turbo+").font(.turboCaption.weight(.bold))
                        }
                        .foregroundStyle(t.orange)
                        .padding(.horizontal, 9).padding(.vertical, 3)
                        .background(t.orange.opacity(0.15), in: Capsule())
                    } else {
                        Text("Not signed in").font(.turboSubhead).foregroundStyle(t.label2)
                    }
                }
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold)).foregroundStyle(t.label3)
            }
            .padding(.horizontal, 18)
            .padding(.bottom, 16)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("menu.account")
    }

    private var friendCode: some View {
        HStack(spacing: 10) {
            Image(systemName: "qrcode").font(.system(size: 22)).foregroundStyle(t.label)
            VStack(alignment: .leading, spacing: 1) {
                Text("Friend code").font(.turboFootnote).foregroundStyle(t.label2)
                Text("TURBO-4K9X").font(.turboHeadline).foregroundStyle(t.label)
            }
            Spacer()
            Text("Share")
                .font(.turboSubhead.weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, 13).padding(.vertical, 6)
                .background(t.blue, in: Capsule())
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

    private func menuRow(_ route: RootView.Route, _ title: String, _ symbol: String, _ color: Color, _ subtitle: String?) -> some View {
        Button {
            dismiss()
            onSelect(route)
        } label: {
            HStack(spacing: 12) {
                Glyph(symbol: symbol, color: color, size: 29, cornerRadius: 7)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.turboBody).foregroundStyle(t.label)
                    if let subtitle {
                        Text(subtitle).font(.turboFootnote).foregroundStyle(t.label2)
                    }
                }
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold)).foregroundStyle(t.label3)
            }
            .padding(.horizontal, 14).padding(.vertical, 11)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("menu.\(route)")
    }
}
