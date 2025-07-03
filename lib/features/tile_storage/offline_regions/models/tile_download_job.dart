import 'package:flutter/foundation.dart';

enum TileJobStatus { pending, inProgress, success, failed }

@immutable
class TileDownloadJob {
  final String regionId;
  final String providerId;
  final int z;
  final int x;
  final int y;
  final String url; // Pre-calculated URL for the worker
  final TileJobStatus status;
  final int attemptCount;
  final String? workerId;
  final DateTime? startedAt;

  const TileDownloadJob({
    required this.regionId,
    required this.providerId,
    required this.z,
    required this.x,
    required this.y,
    required this.url,
    this.status = TileJobStatus.pending,
    this.attemptCount = 0,
    this.workerId,
    this.startedAt,
  });

  Map<String, dynamic> toNewJobMap() {
    return {
      'regionId': regionId,
      'providerId': providerId,
      'z': z,
      'x': x,
      'y': y,
      'url': url,
      'status': TileJobStatus.pending.index,
      'attemptCount': 0,
    };
  }

  factory TileDownloadJob.fromMap(Map<String, dynamic> map) {
    return TileDownloadJob(
      regionId: map['regionId'],
      providerId: map['providerId'],
      z: map['z'],
      x: map['x'],
      y: map['y'],
      url: map['url'],
      status: TileJobStatus.values[map['status']],
      attemptCount: map['attemptCount'],
      workerId: map['workerId'],
      startedAt:
      map['startedAt'] != null ? DateTime.parse(map['startedAt']) : null,
    );
  }

  TileDownloadJob copyWith({
    TileJobStatus? status,
    int? attemptCount,
    String? workerId,
    DateTime? startedAt,
  }) {
    return TileDownloadJob(
      regionId: regionId,
      providerId: providerId,
      z: z,
      x: x,
      y: y,
      url: url,
      status: status ?? this.status,
      attemptCount: attemptCount ?? this.attemptCount,
      workerId: workerId, // copyWith doesn't clear if null is passed
      startedAt: startedAt,
    );
  }
}