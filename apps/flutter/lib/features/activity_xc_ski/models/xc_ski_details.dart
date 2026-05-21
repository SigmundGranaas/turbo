enum XcSkiTechnique { classic, skate, both, backcountry }
enum GroomingStatus { unknown, today, yesterday, olderThanTwoDays, neverGroomed }

class XcSkiDetails {
  final int distanceMeters;
  final int ascentMeters;
  final int descentMeters;
  final XcSkiTechnique technique;
  final GroomingStatus groomingStatus;
  final bool isLit;
  final bool requiresSeasonPass;
  final String? groomingFeedKey;

  const XcSkiDetails({
    required this.distanceMeters,
    required this.ascentMeters,
    required this.descentMeters,
    required this.technique,
    this.groomingStatus = GroomingStatus.unknown,
    this.isLit = false,
    this.requiresSeasonPass = false,
    this.groomingFeedKey,
  });

  Map<String, dynamic> toJson() => {
        'distanceMeters': distanceMeters,
        'ascentMeters': ascentMeters,
        'descentMeters': descentMeters,
        'technique': technique.index,
        'groomingStatus': groomingStatus.index,
        'isLit': isLit,
        'requiresSeasonPass': requiresSeasonPass,
        'groomingFeedKey': ?groomingFeedKey,
      };

  factory XcSkiDetails.fromJson(Map<String, dynamic> json) => XcSkiDetails(
        distanceMeters: (json['distanceMeters'] as num).toInt(),
        ascentMeters: (json['ascentMeters'] as num).toInt(),
        descentMeters: (json['descentMeters'] as num).toInt(),
        technique: XcSkiTechnique.values[(json['technique'] as num).toInt()],
        groomingStatus: GroomingStatus.values[(json['groomingStatus'] as num).toInt()],
        isLit: json['isLit'] as bool? ?? false,
        requiresSeasonPass: json['requiresSeasonPass'] as bool? ?? false,
        groomingFeedKey: json['groomingFeedKey'] as String?,
      );
}
