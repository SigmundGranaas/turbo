/// Mutable draft an [ActivityObservationForm] hands to a per-kind extras
/// widget. The form owns the always-required fields (observedAt, rating,
/// comment, photoCount); the kind widget mutates [kindPayload] freely,
/// and the form serialises everything into a single request payload at
/// submit time.
class ObservationDraft {
  DateTime observedAt;
  int? rating;
  String? comment;
  int photoCount;
  final Map<String, Object?> kindPayload;

  ObservationDraft({
    required this.observedAt,
    this.rating,
    this.comment,
    this.photoCount = 0,
    Map<String, Object?>? kindPayload,
  }) : kindPayload = kindPayload ?? <String, Object?>{};
}
