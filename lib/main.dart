import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:map_app/widgets/map/main_map.dart';

import 'data/datastore/factory.dart';
import 'data/state/providers/initialize_tiles_provider.dart';

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
    // Initialize tiles
    ref.watch(initializeTilesProvider);

    if (kDebugMode) {
      print("Building MyApp");
    }

    return MaterialApp(
      title: 'Turbo',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color.fromRGBO(0, 95, 126, 100)),
        useMaterial3: true,
        textTheme: GoogleFonts.nunitoSansTextTheme(),
      ),
      // Go directly to the map screen
      home: const MapControllerPage(),
    );
  }
}