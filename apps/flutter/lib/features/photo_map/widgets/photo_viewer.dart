import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:photo_manager/photo_manager.dart' hide LatLng;

import '../models/photo_location.dart';
import 'photo_thumbnail.dart';

/// Full-screen viewer for a single geotagged photo. Loads a large preview
/// (not the full original) to stay memory-safe, and supports pinch-zoom.
Future<void> showPhotoViewer(BuildContext context, PhotoLocation photo) {
  return Navigator.of(context).push(
    MaterialPageRoute<void>(
      fullscreenDialog: true,
      builder: (_) => _PhotoViewerPage(photo: photo),
    ),
  );
}

/// Bottom-sheet grid of the photos in a cluster. Tapping one opens the
/// full-screen viewer. Used when a cluster can't be expanded further by
/// zooming (already at high zoom) or as the generic "show all" action.
Future<void> showPhotoClusterSheet(
  BuildContext context,
  List<PhotoLocation> photos,
) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (ctx) => _PhotoClusterSheet(photos: photos),
  );
}

class _PhotoViewerPage extends StatelessWidget {
  final PhotoLocation photo;
  const _PhotoViewerPage({required this.photo});

  @override
  Widget build(BuildContext context) {
    final created = photo.createdAt;
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        backgroundColor: Colors.black,
        foregroundColor: Colors.white,
        title: created != null
            ? Text(DateFormat.yMMMMd().add_jm().format(created))
            : null,
      ),
      body: Center(
        child: InteractiveViewer(
          maxScale: 5,
          child: FutureBuilder<Uint8List?>(
            future: photo.asset.thumbnailDataWithSize(
              const ThumbnailSize(1080, 1080),
            ),
            builder: (context, snapshot) {
              final bytes = snapshot.data;
              if (bytes == null) {
                return const CircularProgressIndicator(color: Colors.white);
              }
              return Image.memory(bytes, fit: BoxFit.contain);
            },
          ),
        ),
      ),
    );
  }
}

class _PhotoClusterSheet extends StatelessWidget {
  final List<PhotoLocation> photos;
  const _PhotoClusterSheet({required this.photos});

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.6,
      maxChildSize: 0.95,
      builder: (context, scrollController) {
        return Material(
          color: colorScheme.surface,
          borderRadius:
              const BorderRadius.vertical(top: Radius.circular(24)),
          clipBehavior: Clip.antiAlias,
          child: Column(
            children: [
              const SizedBox(height: 12),
              Center(
                child: Container(
                  width: 32,
                  height: 4,
                  decoration: BoxDecoration(
                    color: colorScheme.outlineVariant,
                    borderRadius: const BorderRadius.all(Radius.circular(2)),
                  ),
                ),
              ),
              Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  children: [
                    Icon(Icons.photo_library_outlined,
                        color: colorScheme.primary),
                    const SizedBox(width: 12),
                    Text(
                      '${photos.length}',
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                  ],
                ),
              ),
              Expanded(
                child: GridView.builder(
                  controller: scrollController,
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  gridDelegate:
                      const SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: 3,
                    mainAxisSpacing: 4,
                    crossAxisSpacing: 4,
                  ),
                  itemCount: photos.length,
                  itemBuilder: (context, index) {
                    final photo = photos[index];
                    return GestureDetector(
                      onTap: () => showPhotoViewer(context, photo),
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(8),
                        child: PhotoThumbnail(
                          asset: photo.asset,
                          size: double.infinity,
                        ),
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
