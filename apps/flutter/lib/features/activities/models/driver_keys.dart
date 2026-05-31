/// The whitelist of driver keys the per-kind orchestrators emit and
/// the detail sheets pluck out. These mirror the .NET orchestrator's
/// `Driver.Key` values — first time backend renames one, this file
/// breaks every sheet that referenced it, instead of the metric
/// silently vanishing.
///
/// New drivers: add the constant here, then route every UI reference
/// through it. The contract test
/// `test/features/activities/driver_keys_contract_test.dart` enforces
/// that no sheet references a string outside this set.
abstract final class DriverKeys {
  // Cross-kind weather drivers ---------------------------------------
  static const tempBand = 'temp_band';
  static const wind = 'wind';
  static const rain24h = 'rain_24h';
  static const pressureTrend = 'pressure_trend';

  // Snow / winter -----------------------------------------------------
  static const freshSnow24h = 'fresh_snow_24h';
  static const snowDepth = 'snow_depth';

  // Water / marine ----------------------------------------------------
  static const seaTemp = 'sea_temp';
  static const waterTemp = 'water_temp';
  static const waveHeight = 'wave_height';
  static const vizMeters = 'viz_meters';
  static const flowCumecs = 'flow_cumecs';

  /// All keys recognised by the UI. Kept in sync by the contract test.
  static const known = <String>{
    tempBand,
    wind,
    rain24h,
    pressureTrend,
    freshSnow24h,
    snowDepth,
    seaTemp,
    waterTemp,
    waveHeight,
    vizMeters,
    flowCumecs,
  };
}
