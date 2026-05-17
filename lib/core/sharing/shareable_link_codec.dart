import 'dart:convert';

import 'package:archive/archive.dart';
import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:latlong2/latlong.dart';

import '../../features/markers/api.dart';
import '../../features/saved_paths/api.dart';

/// Maximum length of a share URL before we refuse to build it.
/// 8000 chars is a safe upper bound across modern browsers (Edge caps URLs
/// near 2 KB; Chrome/Firefox tolerate ~32 KB; servers and proxies often cap
/// near 8 KB). Past this we steer users toward GPX export.
const int kMaxShareUrlLength = 8000;

/// Current schema version of the encoded payload.
const int _kPayloadVersion = 1;

/// Marker kind in URLs: `/share/m`.
const String _kMarkerKind = 'm';

/// Path kind in URLs: `/share/p`.
const String _kPathKind = 'p';

/// Result of decoding a share URL.
sealed class SharedPayload {
  const SharedPayload();
}

class SharedMarkerPayload extends SharedPayload {
  final Marker marker;

  /// Optional collection uuid to add the imported marker to. Carried via the
  /// share URL so the recipient sees a "Will be added to" hint and the
  /// payload listener can wire up membership in one step.
  final String? targetCollectionId;
  const SharedMarkerPayload(this.marker, {this.targetCollectionId});
}

class SharedPathPayload extends SharedPayload {
  final SavedPath path;
  final String? targetCollectionId;
  const SharedPathPayload(this.path, {this.targetCollectionId});
}

class LinkTooLargeException implements Exception {
  final int length;
  LinkTooLargeException(this.length);
  @override
  String toString() =>
      'LinkTooLargeException: encoded URL is $length chars '
      '(max $kMaxShareUrlLength). Share as GPX instead.';
}

class InvalidShareLinkException implements Exception {
  final String message;
  InvalidShareLinkException(this.message);
  @override
  String toString() => 'InvalidShareLinkException: $message';
}

class UnsupportedShareVersionException implements Exception {
  final int version;
  UnsupportedShareVersionException(this.version);
  @override
  String toString() =>
      'UnsupportedShareVersionException: payload version $version is not '
      'supported by this client. Update the app to view this share.';
}

/// Encodes markers and saved paths into shareable web URLs and decodes them
/// back. The payload (JSON) is gzipped and base64url-encoded into a single
/// `d` query parameter. The host is the configured Flutter-web frontend.
class ShareableLinkCodec {
  ShareableLinkCodec._();

  static String encodeMarker(Marker marker, String webBaseUrl,
      {String? targetCollectionId}) {
    final json = <String, dynamic>{
      'v': _kPayloadVersion,
      't': marker.title,
      if (_nonEmpty(marker.description)) 'd': marker.description,
      if (_nonEmpty(marker.icon)) 'i': marker.icon,
      if (_nonEmpty(targetCollectionId)) 'col': targetCollectionId,
      'p': [
        _round6(marker.position.latitude),
        _round6(marker.position.longitude),
      ],
    };
    return _buildUrl(webBaseUrl, _kMarkerKind, json);
  }

  static String encodePath(SavedPath path, String webBaseUrl,
      {String? targetCollectionId}) {
    final json = <String, dynamic>{
      'v': _kPayloadVersion,
      't': path.title,
      if (_nonEmpty(path.description)) 'd': path.description,
      if (_nonEmpty(path.colorHex)) 'c': path.colorHex,
      if (_nonEmpty(path.iconKey)) 'i': path.iconKey,
      if (path.smoothing) 's': true,
      if (_nonEmpty(path.lineStyleKey)) 'l': path.lineStyleKey,
      if (_nonEmpty(targetCollectionId)) 'col': targetCollectionId,
      'pts': [
        for (final pt in path.points)
          [_round6(pt.latitude), _round6(pt.longitude)],
      ],
    };
    return _buildUrl(webBaseUrl, _kPathKind, json);
  }

  /// Returns `null` if [uri] is not a share URL; throws on malformed shares.
  static SharedPayload? decodeShareUrl(Uri uri) {
    final segments = uri.pathSegments;
    if (segments.length < 2) return null;
    final tail = segments.sublist(segments.length - 2);
    if (tail[0] != 'share') return null;
    final kind = tail[1];
    if (kind != _kMarkerKind && kind != _kPathKind) return null;

    final data = uri.queryParameters['d'];
    if (data == null || data.isEmpty) {
      throw InvalidShareLinkException('missing "d" query parameter');
    }
    return decodeRawPayload(data, kind: kind);
  }

  /// Test-only entry point that bypasses URL parsing.
  @visibleForTesting
  static SharedPayload decodeRawPayload(
    String data, {
    required String kind,
    int? overrideVersionForTest,
  }) {
    final json = _decodeJson(data);
    final version = overrideVersionForTest ?? (json['v'] as int? ?? -1);
    if (version != _kPayloadVersion) {
      throw UnsupportedShareVersionException(version);
    }
    final col = json['col'] as String?;
    return switch (kind) {
      _kMarkerKind =>
        SharedMarkerPayload(_markerFromJson(json), targetCollectionId: col),
      _kPathKind =>
        SharedPathPayload(_pathFromJson(json), targetCollectionId: col),
      _ => throw InvalidShareLinkException('unknown share kind "$kind"'),
    };
  }

  // ---------------------------------------------------------------------------

  static String _buildUrl(String webBaseUrl, String kind, Map<String, dynamic> json) {
    final encoded = _encodeJson(json);
    final base = Uri.parse(webBaseUrl);
    final newSegments = <String>[
      ...base.pathSegments.where((s) => s.isNotEmpty),
      'share',
      kind,
    ];
    final url = base.replace(
      pathSegments: newSegments,
      queryParameters: {'d': encoded},
    ).toString();

    if (url.length > kMaxShareUrlLength) {
      throw LinkTooLargeException(url.length);
    }
    return url;
  }

  static String _encodeJson(Map<String, dynamic> json) {
    final raw = utf8.encode(jsonEncode(json));
    final gzipped = const GZipEncoder().encode(raw);
    return base64UrlEncode(gzipped).replaceAll('=', '');
  }

  static Map<String, dynamic> _decodeJson(String data) {
    try {
      final padded = _padBase64(data);
      final gzipped = base64Url.decode(padded);
      final raw = const GZipDecoder().decodeBytes(gzipped);
      final decoded = jsonDecode(utf8.decode(raw));
      if (decoded is! Map<String, dynamic>) {
        throw InvalidShareLinkException('payload is not a JSON object');
      }
      return decoded;
    } on InvalidShareLinkException {
      rethrow;
    } catch (e) {
      throw InvalidShareLinkException('could not decode payload: $e');
    }
  }

  static String _padBase64(String s) {
    final pad = (4 - s.length % 4) % 4;
    return s + ('=' * pad);
  }

  static Marker _markerFromJson(Map<String, dynamic> json) {
    final p = json['p'];
    if (p is! List || p.length != 2) {
      throw InvalidShareLinkException('marker payload missing position');
    }
    return Marker(
      title: json['t'] as String? ?? '',
      description: json['d'] as String?,
      icon: json['i'] as String?,
      position: LatLng((p[0] as num).toDouble(), (p[1] as num).toDouble()),
    );
  }

  static SavedPath _pathFromJson(Map<String, dynamic> json) {
    final raw = json['pts'];
    if (raw is! List || raw.isEmpty) {
      throw InvalidShareLinkException('path payload missing points');
    }
    final points = <LatLng>[];
    for (final entry in raw) {
      if (entry is! List || entry.length != 2) {
        throw InvalidShareLinkException('path point malformed');
      }
      points.add(LatLng(
        (entry[0] as num).toDouble(),
        (entry[1] as num).toDouble(),
      ));
    }
    final distance = _haversineDistance(points);
    return SavedPath(
      title: json['t'] as String? ?? '',
      description: json['d'] as String?,
      points: points,
      distance: distance,
      colorHex: json['c'] as String?,
      iconKey: json['i'] as String?,
      smoothing: json['s'] as bool? ?? false,
      lineStyleKey: json['l'] as String?,
    );
  }

  static double _haversineDistance(List<LatLng> points) {
    const distance = Distance();
    var total = 0.0;
    for (var i = 1; i < points.length; i++) {
      total += distance(points[i - 1], points[i]);
    }
    return total;
  }

  static bool _nonEmpty(String? s) => s != null && s.isNotEmpty;

  static double _round6(double v) => (v * 1e6).roundToDouble() / 1e6;
}
