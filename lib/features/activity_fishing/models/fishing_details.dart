import 'package:latlong2/latlong.dart';

/// What kind of water the spot is in. Mirrors the server enum.
enum WaterKind { river, lake, sea }

/// How the spot is fished. Mirrors the server enum.
enum ShoreOrBoat { shore, boat, either }

/// Typed payload of fishing-specific fields. Every property is typed —
/// no map-of-string-to-dynamic catch-all.
class FishingDetails {
  final WaterKind waterKind;
  final ShoreOrBoat shoreOrBoat;
  final String? accessNotes;
  final List<TargetSpecies> targetSpecies;
  final List<DepthSample> knownDepths;
  final PreferredConditions? preferred;

  const FishingDetails({
    required this.waterKind,
    required this.shoreOrBoat,
    this.accessNotes,
    this.targetSpecies = const [],
    this.knownDepths = const [],
    this.preferred,
  });

  FishingDetails copyWith({
    WaterKind? waterKind,
    ShoreOrBoat? shoreOrBoat,
    String? accessNotes,
    List<TargetSpecies>? targetSpecies,
    List<DepthSample>? knownDepths,
    PreferredConditions? preferred,
  }) => FishingDetails(
        waterKind: waterKind ?? this.waterKind,
        shoreOrBoat: shoreOrBoat ?? this.shoreOrBoat,
        accessNotes: accessNotes ?? this.accessNotes,
        targetSpecies: targetSpecies ?? this.targetSpecies,
        knownDepths: knownDepths ?? this.knownDepths,
        preferred: preferred ?? this.preferred,
      );

  Map<String, dynamic> toJson() => {
        'waterKind': waterKind.index,
        'shoreOrBoat': shoreOrBoat.index,
        if (accessNotes != null) 'accessNotes': accessNotes,
        'targetSpecies': targetSpecies.map((t) => t.toJson()).toList(),
        'knownDepths': knownDepths.map((d) => d.toJson()).toList(),
        if (preferred != null) 'preferred': preferred!.toJson(),
      };

  factory FishingDetails.fromJson(Map<String, dynamic> json) => FishingDetails(
        waterKind: WaterKind.values[(json['waterKind'] as num).toInt()],
        shoreOrBoat: ShoreOrBoat.values[(json['shoreOrBoat'] as num).toInt()],
        accessNotes: json['accessNotes'] as String?,
        targetSpecies: (json['targetSpecies'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(TargetSpecies.fromJson)
                .toList() ??
            const [],
        knownDepths: (json['knownDepths'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(DepthSample.fromJson)
                .toList() ??
            const [],
        preferred: json['preferred'] is Map<String, dynamic>
            ? PreferredConditions.fromJson(json['preferred'] as Map<String, dynamic>)
            : null,
      );
}

class TargetSpecies {
  final String speciesCode;
  final String? notes;
  const TargetSpecies({required this.speciesCode, this.notes});

  Map<String, dynamic> toJson() => {
        'speciesCode': speciesCode,
        if (notes != null) 'notes': notes,
      };

  factory TargetSpecies.fromJson(Map<String, dynamic> json) => TargetSpecies(
        speciesCode: json['speciesCode'] as String,
        notes: json['notes'] as String?,
      );
}

class DepthSample {
  final LatLng position;
  final double depthMeters;
  const DepthSample({required this.position, required this.depthMeters});

  Map<String, dynamic> toJson() => {
        'lat': position.latitude,
        'lon': position.longitude,
        'depthMeters': depthMeters,
      };

  factory DepthSample.fromJson(Map<String, dynamic> json) => DepthSample(
        position: LatLng(
          (json['lat'] as num).toDouble(),
          (json['lon'] as num).toDouble(),
        ),
        depthMeters: (json['depthMeters'] as num).toDouble(),
      );
}

class PreferredConditions {
  final int? pressureMinHpa;
  final int? pressureMaxHpa;
  final double? windMaxMs;
  const PreferredConditions({this.pressureMinHpa, this.pressureMaxHpa, this.windMaxMs});

  Map<String, dynamic> toJson() => {
        if (pressureMinHpa != null) 'pressureMinHpa': pressureMinHpa,
        if (pressureMaxHpa != null) 'pressureMaxHpa': pressureMaxHpa,
        if (windMaxMs != null) 'windMaxMs': windMaxMs,
      };

  factory PreferredConditions.fromJson(Map<String, dynamic> json) => PreferredConditions(
        pressureMinHpa: (json['pressureMinHpa'] as num?)?.toInt(),
        pressureMaxHpa: (json['pressureMaxHpa'] as num?)?.toInt(),
        windMaxMs: (json['windMaxMs'] as num?)?.toDouble(),
      );
}
