/// Varsom (NVE) regional avalanche danger level, 1–5.
enum AvalancheDangerLevel {
  /// Level 1 — generally safe conditions.
  low,

  /// Level 2 — heightened avalanche conditions on specific slopes.
  moderate,

  /// Level 3 — dangerous; careful route selection required.
  considerable,

  /// Level 4 — very dangerous; avoid avalanche terrain.
  high,

  /// Level 5 — extreme; avoid all avalanche terrain.
  extreme;

  int get numeric => index + 1;

  static AvalancheDangerLevel? fromNumeric(int? raw) {
    switch (raw) {
      case 1:
        return AvalancheDangerLevel.low;
      case 2:
        return AvalancheDangerLevel.moderate;
      case 3:
        return AvalancheDangerLevel.considerable;
      case 4:
        return AvalancheDangerLevel.high;
      case 5:
        return AvalancheDangerLevel.extreme;
      default:
        return null;
    }
  }
}

/// One avalanche problem inside a regional forecast.
class AvalancheProblem {
  final String? typeName;
  final String? sensitivity;
  final String? distribution;
  final String? size;

  const AvalancheProblem({
    required this.typeName,
    required this.sensitivity,
    required this.distribution,
    required this.size,
  });
}

/// Daily regional avalanche forecast returned by Varsom for the point queried.
///
/// Varsom only covers Norwegian mountainous regions. Outside that footprint
/// the API responds with an empty array — represented here as a `null`
/// [AvalancheWarning] at the notifier layer rather than constructing an
/// empty warning.
class AvalancheWarning {
  final int regionId;
  final String regionName;
  final DateTime validDate;
  final AvalancheDangerLevel dangerLevel;
  final String? mainText;
  final String? avalancheDanger;
  final List<AvalancheProblem> problems;

  const AvalancheWarning({
    required this.regionId,
    required this.regionName,
    required this.validDate,
    required this.dangerLevel,
    required this.mainText,
    required this.avalancheDanger,
    required this.problems,
  });
}
