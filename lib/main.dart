import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart' as offline_regions;
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/core/theme/theme.dart';
import 'package:turbo/core/theme/utils.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/markers/api.dart';

import 'core/data/database_provider.dart';
import 'core/service/logger.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  setupLogging();
  
  final container = ProviderContainer();
  
  // Trigger background initializations without awaiting them.
  // This allows the app to start rendering immediately.
  unawaited(_backgroundInit(container));

  runApp(
    UncontrolledProviderScope(
      container: container,
      child: const MyApp(),
    ),
  );
}

Future<void> _backgroundInit(ProviderContainer container) async {
  if (!kIsWeb) {
    // These calls are now fire-and-forget background tasks.
    container.read(databaseProvider);
    // We LISTEN here because the provider might be null initially while DB is loading.
    // Listening ensures it gets created and started as soon as dependencies are ready.
    container.listen(offline_regions.downloadOrchestratorProvider, (_, _) {});
  }
  // This triggers the internal build() logic which starts the session check
  container.read(authStateProvider);
  container.read(localMarkerDataStoreProvider);
}

class MyApp extends ConsumerWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final authState = ref.watch(authStateProvider);
    final settingsAsync = ref.watch(settingsProvider);

    if (kDebugMode) {
      print("Building MyApp. Auth Status: ${authState.status}, Initializing: ${authState.isInitializing}");
    }
    TextTheme textTheme =
    createTextTheme(context, "Roboto", "Libre Baskerville");
    MaterialTheme theme = MaterialTheme(textTheme);

    return MaterialApp(
      onGenerateTitle: (context) => AppLocalizations.of(context).appTitle,
      debugShowCheckedModeBanner: false,
      theme: theme.light(),
      darkTheme: theme.dark(),
      themeMode: settingsAsync.value?.themeMode ?? ThemeMode.system,
      locale: settingsAsync.value?.locale ?? const Locale('en'),
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      routes: {
        '/': (context) => const MainMapPage(),
        '/login/success': (context) => const LoginSuccessPage(),
        '/login/callback': (context) => const GoogleAuthCallbackPage(),
      },
      initialRoute: '/',
    );
  }
}