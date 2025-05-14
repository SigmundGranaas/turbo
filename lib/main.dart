import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/theme.dart';
import 'package:map_app/utils.dart';
import 'package:map_app/widgets/auth/google_oauth_screen.dart';
import 'package:map_app/widgets/auth/login_success.dart';
import 'package:map_app/widgets/map/main_map.dart';

import 'data/state/providers/initialize_tiles_provider.dart';
import 'data/auth/auth_providers.dart'; // For authStateProvider
import 'data/auth/auth_init_provider.dart'; // For linkStreamHandlerProvider
import 'data/state/providers/location_repository.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(
    const ProviderScope(
      child: MyApp(),
    ),
  );
}

class MyApp extends ConsumerStatefulWidget { // Changed to ConsumerStatefulWidget
  const MyApp({super.key});

  @override
  ConsumerState<MyApp> createState() => _MyAppState();
}

class _MyAppState extends ConsumerState<MyApp> { // Changed to ConsumerState

  @override
  void initState() {
    super.initState();
    // Initialize auth state and handle initial link/callback.
    // Done in Future.microtask to ensure it runs after first frame/build.
    Future.microtask(() {
      ref.read(authStateProvider.notifier).initializeAndHandleInitialLink();
      // Activate the subsequent link listener.
      ref.read(linkStreamHandlerProvider);
    });

    // Eagerly initialize the local marker data store via its provider.
    Future.microtask(() => ref.read(localMarkerDataStoreProvider).init());
  }

  @override
  Widget build(BuildContext context) { // WidgetRef is implicitly available as `ref`
    // Initialize tiles (this FutureProvider will be kept alive)
    ref.watch(initializeTilesProvider);

    // Watch authStateProvider to rebuild MyApp or parts of it when auth changes.
    // This line itself doesn't cause issues, it's the modification during init that did.
    final _ = ref.watch(authStateProvider);

    if (kDebugMode) {
      print("Building MyApp. Auth Status: ${ref.read(authStateProvider).status}");
    }
    TextTheme textTheme = createTextTheme(context, "Roboto", "Libre Baskerville");
    MaterialTheme theme = MaterialTheme(textTheme);

    return MaterialApp(
      title: 'Turbo',
      debugShowCheckedModeBanner: false,
      theme:  theme.light(),
      darkTheme: theme.dark(),
      themeMode: ThemeMode.system,
      routes: {
        '/': (context) => const MapControllerPage(),
        '/login/callback': (context) => const GoogleAuthCallbackPage(),
        '/login/success': (context) => const LoginSuccessPage(),
      },
      initialRoute: '/',
    );
  }
}