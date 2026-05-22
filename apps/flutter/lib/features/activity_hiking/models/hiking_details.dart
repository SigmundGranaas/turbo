import 'package:latlong2/latlong.dart';

enum HikingDifficulty { easy, moderate, hard, expert }
enum TrailMarking { unmarked, cairns, paint, signposted }
enum TrailSurface { mixed, path, gravel, boardwalk, rock, scree, snow }

class HikingDetails {
  final int distanceMeters;
  final int ascentMeters;
  final int descentMeters;
  final int elevationMinMeters;
  final int elevationMaxMeters;
  final HikingDifficulty difficulty;
  final TrailSurface surface;
  final TrailMarking marking;
  final double? estimatedHours;
  final bool hasWaterSources;
  final bool hasShelter;
  final List<WaterSource> waterSources;

  const HikingDetails({
    required this.distanceMeters,
    required this.ascentMeters,
    required this.descentMeters,
    required this.elevationMinMeters,
    required this.elevationMaxMeters,
    required this.difficulty,
    required this.surface,
    required this.marking,
    this.estimatedHours,
    this.hasWaterSources = false,
    this.hasShelter = false,
    this.waterSources = const [],
  });

  Map<String, dynamic> toJson() => {
        'distanceMeters': distanceMeters,
        'ascentMeters': ascentMeters,
        'descentMeters': descentMeters,
        'elevationMinMeters': elevationMinMeters,
        'elevationMaxMeters': elevationMaxMeters,
        'difficulty': difficulty.index,
        'surface': surface.index,
        'marking': marking.index,
        'estimatedHours': ?estimatedHours,
        'hasWaterSources': hasWaterSources,
        'hasShelter': hasShelter,
        'waterSources': waterSources.map((w) => w.toJson()).toList(),
      };

  factory HikingDetails.fromJson(Map<String, dynamic> json) => HikingDetails(
        distanceMeters: (json['distanceMeters'] as num).toInt(),
        ascentMeters: (json['ascentMeters'] as num).toInt(),
        descentMeters: (json['descentMeters'] as num).toInt(),
        elevationMinMeters: (json['elevationMinMeters'] as num).toInt(),
        elevationMaxMeters: (json['elevationMaxMeters'] as num).toInt(),
        difficulty: HikingDifficulty.values[(json['difficulty'] as num).toInt()],
        surface: TrailSurface.values[(json['surface'] as num).toInt()],
        marking: TrailMarking.values[(json['marking'] as num).toInt()],
        estimatedHours: (json['estimatedHours'] as num?)?.toDouble(),
        hasWaterSources: json['hasWaterSources'] as bool? ?? false,
        hasShelter: json['hasShelter'] as bool? ?? false,
        waterSources: (json['waterSources'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(WaterSource.fromJson)
                .toList() ??
            const [],
      );
}

class WaterSource {
  final LatLng position;
  final String kind;
  final String? notes;
  const WaterSource({required this.position, required this.kind, this.notes});

  Map<String, dynamic> toJson() => {
        'lat': position.latitude,
        'lon': position.longitude,
        'kind': kind,
        'notes': ?notes,
      };

  factory WaterSource.fromJson(Map<String, dynamic> json) => WaterSource(
        position: LatLng(
          (json['lat'] as num).toDouble(),
          (json['lon'] as num).toDouble(),
        ),
        kind: json['kind'] as String,
        notes: json['notes'] as String?,
      );
}
