import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/download_orchestrator.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/theme.dart';
import 'package:turbo/utils.dart';
import 'package:turbo/widgets/auth/google_oauth_screen.dart';
import 'package:turbo/widgets/auth/login_success.dart';

import 'core/data/database_provider.dart';
import 'core/service/logger.dart';
import 'data/auth/auth_providers.dart';
import 'data/auth/auth_init_provider.dart';
import 'data/state/providers/location_repository.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  setupLogging();
  runApp(
    const ProviderScope(
      child: MyApp(),
    ),
  );
}

class MyApp extends ConsumerStatefulWidget {
  const MyApp({super.key});

  @override
  ConsumerState<MyApp> createState() => _MyAppState();
}

class _MyAppState extends ConsumerState<MyApp> {
  @override
  void initState() {
    super.initState();
    // Eagerly initialize providers on startup.
    // This ensures the database is created and services are ready.
    Future.microtask(() {
      if (!kIsWeb) {
        ref.read(databaseProvider);
        ref.read(downloadOrchestratorProvider);
      }
      // These providers are safe for all platforms.
      ref.read(authStateProvider.notifier).initializeAndHandleInitialLink();
      ref.read(linkStreamHandlerProvider);
      ref.read(localMarkerDataStoreProvider);
    });
  }

  @override
  Widget build(BuildContext context) {
    final authState = ref.watch(authStateProvider);
    final settingsAsync = ref.watch(settingsProvider);

    if (kDebugMode) {
      print("Building MyApp. Auth Status: ${authState.status}");
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