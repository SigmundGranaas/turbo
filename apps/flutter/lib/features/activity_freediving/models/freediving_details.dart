enum WaterBody { sea, fjord, lake }
enum BottomType { unknown, sandyShallow, rockyShallow, kelpForest, wall, reef, seagrassMeadow, open }

class FreedivingDetails {
  final WaterBody waterBody;
  final BottomType bottomType;
  final double maxDepthMeters;
  final double? typicalVisibilityMeters;
  final bool harpoonAllowed;
  final bool shoreEntry;
  final String? accessNotes;
  final List<TargetSpecies> targetSpecies;

  const FreedivingDetails({
    required this.waterBody,
    required this.bottomType,
    required this.maxDepthMeters,
    this.typicalVisibilityMeters,
    this.harpoonAllowed = false,
    this.shoreEntry = true,
    this.accessNotes,
    this.targetSpecies = const [],
  });

  Map<String, dynamic> toJson() => {
        'waterBody': waterBody.index,
        'bottomType': bottomType.index,
        'maxDepthMeters': maxDepthMeters,
        'typicalVisibilityMeters': ?typicalVisibilityMeters,
        'harpoonAllowed': harpoonAllowed,
        'shoreEntry': shoreEntry,
        'accessNotes': ?accessNotes,
        'targetSpecies': targetSpecies.map((t) => t.toJson()).toList(),
      };

  factory FreedivingDetails.fromJson(Map<String, dynamic> json) => FreedivingDetails(
        waterBody: WaterBody.values[(json['waterBody'] as num).toInt()],
        bottomType: BottomType.values[(json['bottomType'] as num).toInt()],
        maxDepthMeters: (json['maxDepthMeters'] as num).toDouble(),
        typicalVisibilityMeters: (json['typicalVisibilityMeters'] as num?)?.toDouble(),
        harpoonAllowed: json['harpoonAllowed'] as bool? ?? false,
        shoreEntry: json['shoreEntry'] as bool? ?? true,
        accessNotes: json['accessNotes'] as String?,
        targetSpecies: (json['targetSpecies'] as List?)
                ?.cast<Map<String, dynamic>>()
                .map(TargetSpecies.fromJson)
                .toList() ?? const [],
      );
}

class TargetSpecies {
  final String speciesCode;
  final String? notes;
  const TargetSpecies({required this.speciesCode, this.notes});

  Map<String, dynamic> toJson() => {'speciesCode': speciesCode, 'notes': ?notes};
  factory TargetSpecies.fromJson(Map<String, dynamic> json) => TargetSpecies(
        speciesCode: json['speciesCode'] as String,
        notes: json['notes'] as String?,
      );
}
