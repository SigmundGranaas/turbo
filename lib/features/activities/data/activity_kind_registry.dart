import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../models/activity_kind_descriptor.dart';

/// Composition root for activity kinds. Each kind feature exports a
/// single [ActivityKindDescriptor] and registers it here at app startup.
/// The shell never names a specific kind — it iterates the registry.
class ActivityKindRegistry {
  final Map<String, ActivityKindDescriptor> _byKey;
  final List<ActivityKindDescriptor> _ordered;

  ActivityKindRegistry(Iterable<ActivityKindDescriptor> descriptors)
      : _byKey = {for (final d in descriptors) d.key: d},
        _ordered = descriptors.toList(growable: false);

  List<ActivityKindDescriptor> get all => List.unmodifiable(_ordered);

  ActivityKindDescriptor? get(String kindKey) => _byKey[kindKey];
}

/// App-wide registry. The host wiring (typically `app.dart`) overrides
/// this provider with the list of descriptors the build ships.
final activityKindRegistryProvider = Provider<ActivityKindRegistry>((ref) {
  return ActivityKindRegistry(const []);
});
