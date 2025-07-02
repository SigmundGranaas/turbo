import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'dart:io';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/theme.dart';
import 'package:turbo/utils.dart';
import 'package:turbo/widgets/auth/google_oauth_screen.dart';
import 'package:turbo/widgets/auth/login_success.dart';
import 'package:turbo/widgets/map/main_map.dart';

import 'data/state/providers/initialize_tiles_provider.dart';
import 'data/auth/auth_providers.dart';
import 'data/auth/auth_init_provider.dart';
import 'data/state/providers/location_repository.dart';

import 'package:firebase_core/firebase_core.dart';
import 'firebase_options.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  if(kIsWeb){
    await Firebase.initializeApp(
      options: DefaultFirebaseOptions.currentPlatform,
    );
  } else if(!Platform.isLinux) {
    await Firebase.initializeApp(
      options: DefaultFirebaseOptions.currentPlatform,
    );
  }

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
    Future.microtask(() {
      ref.read(authStateProvider.notifier).initializeAndHandleInitialLink();
      ref.read(linkStreamHandlerProvider);
      ref.read(localMarkerDataStoreProvider);
    });
  }

  @override
  Widget build(BuildContext context) {
    ref.watch(initializeTilesProvider);
    final authState = ref.watch(authStateProvider);
    final settingsAsync = ref.watch(settingsProvider);

    if (kDebugMode) {
      print("Building MyApp. Auth Status: ${authState.status}");
    }
    TextTheme textTheme = createTextTheme(context, "Roboto", "Libre Baskerville");
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
        '/': (context) => const MapControllerPage(),
        '/login/success': (context) => const LoginSuccessPage(),
        '/login/callback': (context) => const GoogleAuthCallbackPage(),
      },
      initialRoute: '/',
    );
  }
}