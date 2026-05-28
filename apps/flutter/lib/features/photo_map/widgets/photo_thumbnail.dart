import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:photo_manager/photo_manager.dart' hide LatLng;

/// Process-wide cache of decoded thumbnails keyed by asset id. Map pans and
/// zooms rebuild the marker layer constantly; without this each rebuild would
/// re-decode every visible thumbnail. Bytes are small (square 150px) so a
/// plain map is fine for the lifetime of the app session.
final Map<String, Uint8List> _thumbCache = {};

/// A square thumbnail for a photo-library [asset]. Loads once, then serves
/// from [_thumbCache] on subsequent rebuilds. Shows a neutral placeholder
/// while loading or if the asset can't be decoded.
class PhotoThumbnail extends StatefulWidget {
  final AssetEntity asset;
  final double size;
  final BoxFit fit;

  const PhotoThumbnail({
    super.key,
    required this.asset,
    required this.size,
    this.fit = BoxFit.cover,
  });

  @override
  State<PhotoThumbnail> createState() => _PhotoThumbnailState();
}

class _PhotoThumbnailState extends State<PhotoThumbnail> {
  Uint8List? _bytes;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final cached = _thumbCache[widget.asset.id];
    if (cached != null) {
      _bytes = cached;
      return;
    }
    final data = await widget.asset.thumbnailDataWithSize(
      const ThumbnailSize.square(150),
    );
    if (data != null) {
      _thumbCache[widget.asset.id] = data;
    }
    if (mounted) {
      setState(() => _bytes = data);
    }
  }

  @override
  Widget build(BuildContext context) {
    final bytes = _bytes;
    if (bytes == null) {
      return Container(
        width: widget.size,
        height: widget.size,
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        child: Icon(
          Icons.photo_outlined,
          size: widget.size * 0.4,
          color: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
      );
    }
    return Image.memory(
      bytes,
      width: widget.size,
      height: widget.size,
      fit: widget.fit,
      gaplessPlayback: true,
    );
  }
}
