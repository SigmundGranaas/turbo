import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/app_theme.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/settings/api.dart';

/// The root [MaterialApp] for the Turbo app.
///
/// Owns theme, localization, and routing. Initialization and provider-
/// container setup live in `main.dart`; everything else about how the app
/// presents itself lives here.
class TurboApp extends ConsumerWidget {
  const TurboApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final authState = ref.watch(authStateProvider);
    final settingsAsync = ref.watch(settingsProvider);

    log.fine(() =>
        'Building TurboApp. Auth Status: ${authState.status}, '
        'Initializing: ${authState.isInitializing}');

    final textTheme = createTextTheme(context, 'Roboto');
    final theme = MaterialTheme(textTheme);

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
      initialRoute: '/',
      routes: {
        '/': (context) => const MainMapPage(),
        '/login/success': (context) => const LoginSuccessPage(),
        '/login/callback': (context) => const GoogleAuthCallbackPage(),
      },
    );
  }
}
