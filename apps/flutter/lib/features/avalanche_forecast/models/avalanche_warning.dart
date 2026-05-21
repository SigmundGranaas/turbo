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

/// Heuristic for whether an avalanche [warning] is worth surfacing to the
/// user in the weather sheet.
///
/// Varsom's coverage is regional — a "Moderate (2)" warning for a region
/// like Salten can legitimately fire all the way into late spring because
/// the high mountains still hold snow. For a user standing in a forested
/// valley in the same region the same warning is noise. This function
/// applies two gates:
///
///  - Level 1 ("Low") is suppressed entirely — Varsom describes it as
///    "generally safe" and a banner just trains the user to ignore them.
///  - Level 2 ("Moderate") is suppressed when the current air temperature
///    at the user's point is warmer than ~5°C, which is a strong signal
///    that the user is below the snow line.
///
/// Levels 3+ always pass through; they're rare and consequential enough to
/// always be worth the screen real estate.
bool shouldShowAvalancheWarning(
  AvalancheWarning warning, {
  required double? currentAirTempC,
}) {
  if (warning.dangerLevel == AvalancheDangerLevel.low) return false;
  if (warning.dangerLevel.numeric >= 3) return true;
  if (currentAirTempC != null && currentAirTempC > 5) return false;
  return true;
}
