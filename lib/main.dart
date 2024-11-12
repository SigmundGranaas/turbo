import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/widgets/map/layers/tiles/providers/initialize_tiles_provider.dart';
import 'package:provider/provider.dart';
import 'data/datastore/factory.dart';
import 'location_provider.dart';
import 'widgets/map/map_controller.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await MarkerDataStoreFactory.init();

  runApp(
    const ProviderScope(
      child: MyApp(),
    ),
  );
}

class MyApp extends ConsumerWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ref.watch(initializeTilesProvider);

    return MaterialApp(
      title: 'Turbo',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.grey),
        useMaterial3: true,
      ),
      home: const MapControllerPage(),
    );
  }
}