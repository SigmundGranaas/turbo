import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_storage/tile_store/data/tile_store_service.dart';

import '../../../core/data/database_provider.dart';

export 'package:turbo/features/tile_storage/tile_store/data/tile_store_service.dart' show TileStoreService;
export 'package:turbo/features/tile_storage/tile_store/models/storage_stats.dart';
export 'package:turbo/features/tile_storage/tile_store/models/tile_record.dart';

/// The public provider for accessing the [TileStoreService].
final tileStoreServiceProvider = FutureProvider<TileStoreService>((ref) async {
  // This will throw if run on web, which is correct as this feature is not for web.
  final db = await ref.watch(databaseProvider.future);
  return TileStoreService(db);
});