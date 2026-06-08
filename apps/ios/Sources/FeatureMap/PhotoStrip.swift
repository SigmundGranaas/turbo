#if canImport(UIKit)
import SwiftUI
import PhotosUI
import UIKit
import CoreModel
import CoreDesignSystem

/// A horizontal strip of a marker's photos with an add-photo tile. iOS-only
/// (uses PhotosPicker + UIImage).
struct PhotoStrip: View {
    @Environment(\.turbo) private var t
    @Bindable var viewModel: MarkerPhotosViewModel
    @State private var pickerItem: PhotosPickerItem?

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 10) {
                PhotosPicker(selection: $pickerItem, matching: .images) {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(t.groupedCard)
                        .frame(width: 80, height: 80)
                        .overlay(
                            VStack(spacing: 4) {
                                Image(systemName: "camera.fill")
                                Text("Add").font(.turboCaption)
                            }
                            .foregroundStyle(t.blue)
                        )
                }
                .accessibilityIdentifier("marker.addphoto")

                ForEach(viewModel.photos) { photo in
                    thumbnail(photo)
                        .contextMenu {
                            Button(role: .destructive) {
                                Task { await viewModel.delete(id: photo.id) }
                            } label: { Label("Delete", systemImage: "trash") }
                        }
                }
            }
        }
        .onChange(of: pickerItem) { _, item in
            guard let item else { return }
            Task {
                if let data = try? await item.loadTransferable(type: Data.self) {
                    await viewModel.add(imageData: data)
                }
                pickerItem = nil
            }
        }
    }

    @ViewBuilder
    private func thumbnail(_ photo: Photo) -> some View {
        if let url = URL(string: photo.uri), let image = UIImage(contentsOfFile: url.path) {
            Image(uiImage: image)
                .resizable()
                .scaledToFill()
                .frame(width: 80, height: 80)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        } else {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(t.fill)
                .frame(width: 80, height: 80)
        }
    }
}
#endif
