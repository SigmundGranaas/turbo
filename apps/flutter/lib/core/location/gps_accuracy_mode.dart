/// User-chosen accuracy/battery tradeoff for live GPS recording. Lives in
/// `core/` so both the settings feature (which persists it) and the
/// path_recording feature (which consumes it) can depend on it without
/// forming a cycle.
enum GpsAccuracyMode {
  high,
  balanced,
  batterySaver;

  static GpsAccuracyMode fromName(String? name) {
    return switch (name) {
      'balanced' => GpsAccuracyMode.balanced,
      'batterySaver' => GpsAccuracyMode.batterySaver,
      _ => GpsAccuracyMode.high,
    };
  }
}
