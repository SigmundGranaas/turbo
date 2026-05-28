import 'package:latlong2/latlong.dart';
// photo_manager ships its own `LatLng`; hide it so the project-wide
// latlong2 type stays unambiguous.
import 'package:photo_manager/photo_manager.dart' hide LatLng;

/// A single geotagged photo from the device library, paired with the
/// [LatLng] where it was taken. Holds a reference to the underlying
/// [AssetEntity] so thumbnails and full-resolution bytes can be loaded
/// lazily when a marker is rendered or opened.
class PhotoLocation {
  final String id;
  final LatLng position;
  final AssetEntity asset;
  final DateTime? createdAt;

  const PhotoLocation({
    required this.id,
    required this.position,
    required this.asset,
    this.createdAt,
  });
}
