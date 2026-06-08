import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

/// A button that lazily mints a share link (an async network call) and then
/// presents the system share sheet for it. Used by marker / hike detail to
/// create a per-resource share link backed by the sharing service. The label is
/// supplied by the caller so it can match the surrounding action row.
public struct ShareLinkButton<Label: View>: View {
    private let create: () async -> URL?
    private let label: () -> Label
    @State private var phase: Phase = .idle
    @State private var sharedURL: SharedURL?

    private enum Phase: Equatable { case idle, loading }

    public init(create: @escaping () async -> URL?, @ViewBuilder label: @escaping () -> Label) {
        self.create = create
        self.label = label
    }

    public var body: some View {
        Button {
            guard phase != .loading else { return }
            phase = .loading
            Task {
                let url = await create()
                phase = .idle
                if let url { sharedURL = SharedURL(url: url) }
            }
        } label: {
            if phase == .loading {
                ProgressView()
            } else {
                label()
            }
        }
        .disabled(phase == .loading)
        .sheet(item: $sharedURL) { wrapper in
            ShareLinkResultSheet(url: wrapper.url)
        }
    }

    private struct SharedURL: Identifiable {
        let url: URL
        var id: String { url.absoluteString }
    }
}

/// Presents a freshly minted link with copy + native share affordances.
private struct ShareLinkResultSheet: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    let url: URL
    @State private var copied = false

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "link.circle.fill").font(.system(size: 52)).foregroundStyle(t.blue)
            Text("Share Link Ready").font(.turboTitle2).foregroundStyle(t.label)
            Text("Anyone with this link can view this place.")
                .font(.turboSubhead).foregroundStyle(t.label2).multilineTextAlignment(.center)

            Text(url.absoluteString)
                .font(.turboFootnote.monospaced()).foregroundStyle(t.label2)
                .lineLimit(1).truncationMode(.middle)
                .padding(12)
                .frame(maxWidth: .infinity)
                .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                .accessibilityIdentifier("share.link.url")

            HStack(spacing: 12) {
                Button {
                    #if canImport(UIKit)
                    UIPasteboard.general.string = url.absoluteString
                    #endif
                    copied = true
                } label: {
                    Label(copied ? "Copied" : "Copy", systemImage: copied ? "checkmark" : "doc.on.doc")
                        .frame(maxWidth: .infinity).padding(.vertical, 12)
                }
                .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                .accessibilityIdentifier("share.link.copy")

                ShareLink(item: url) {
                    Label("Share", systemImage: "square.and.arrow.up")
                        .frame(maxWidth: .infinity).padding(.vertical, 12)
                }
                .background(t.blue, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                .foregroundStyle(.white)
            }
            Spacer()
        }
        .padding(24)
        .background(t.grouped)
        .presentationDetents([.medium])
    }
}
