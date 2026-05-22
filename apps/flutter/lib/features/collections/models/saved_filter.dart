import 'dart:convert';

import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:latlong2/latlong.dart';

/// Declarative criteria for a "smart" collection. Membership is computed
/// from this filter on demand — no cached join table.
///
/// Per the v1 plan this is intentionally limited to a saved search (text +
/// bounding box + date window). A real rule engine, with per-property
/// matchers, is out of scope.
class SavedFilter {
  final String? textQuery;
  final LatLngBounds? boundingBox;
  final DateTime? dateFrom;
  final DateTime? dateTo;

  const SavedFilter({
    this.textQuery,
    this.boundingBox,
    this.dateFrom,
    this.dateTo,
  });

  bool get isEmpty =>
      (textQuery == null || textQuery!.trim().isEmpty) &&
      boundingBox == null &&
      dateFrom == null &&
      dateTo == null;

  factory SavedFilter.fromJson(Map<String, dynamic> json) {
    LatLngBounds? bbox;
    final bboxJson = json['bbox'];
    if (bboxJson is List && bboxJson.length == 4) {
      bbox = LatLngBounds(
        LatLng((bboxJson[0] as num).toDouble(), (bboxJson[1] as num).toDouble()),
        LatLng((bboxJson[2] as num).toDouble(), (bboxJson[3] as num).toDouble()),
      );
    }
    return SavedFilter(
      textQuery: json['q'] as String?,
      boundingBox: bbox,
      dateFrom:
          json['from'] is String ? DateTime.parse(json['from'] as String) : null,
      dateTo: json['to'] is String ? DateTime.parse(json['to'] as String) : null,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      if (textQuery != null && textQuery!.isNotEmpty) 'q': textQuery,
      if (boundingBox != null)
        'bbox': [
          boundingBox!.southWest.latitude,
          boundingBox!.southWest.longitude,
          boundingBox!.northEast.latitude,
          boundingBox!.northEast.longitude,
        ],
      if (dateFrom != null) 'from': dateFrom!.toIso8601String(),
      if (dateTo != null) 'to': dateTo!.toIso8601String(),
    };
  }

  String toJsonString() => jsonEncode(toJson());

  static SavedFilter? fromJsonString(String? raw) {
    if (raw == null || raw.isEmpty) return null;
    try {
      return SavedFilter.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is SavedFilter &&
          textQuery == other.textQuery &&
          boundingBox == other.boundingBox &&
          dateFrom == other.dateFrom &&
          dateTo == other.dateTo;

  @override
  int get hashCode =>
      Object.hash(textQuery, boundingBox, dateFrom, dateTo);
}
