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

class MyApp extends ConsumerStatefulWidget {
  const MyApp({super.key});

  @override
  ConsumerState<MyApp> createState() => _MyAppState();
}

class _MyAppState extends ConsumerState<MyApp> {

  @override
  void initState() {
    super.initState();
    // Use Future.microtask to ensure initialization runs after the first frame.
    Future.microtask(() {
      // Initialize auth state. This handles initial deep links (for mobile OAuth)
      // and checks for existing sessions (for both web and mobile).
      ref.read(authStateProvider.notifier).initializeAndHandleInitialLink();

      // Activate the listener for subsequent deep links (when app is already running).
      ref.read(linkStreamHandlerProvider);

      // Eagerly initialize the local marker data store.
      ref.read(localMarkerDataStoreProvider).init();
    });
  }

  @override
  Widget build(BuildContext context) {
    // Initialize tiles (this FutureProvider will be kept alive by Riverpod).
    ref.watch(initializeTilesProvider);

    // Watch auth state to rebuild when auth status changes.
    final authState = ref.watch(authStateProvider);

    if (kDebugMode) {
      print("Building MyApp. Auth Status: ${authState.status}");
    }
    TextTheme textTheme = createTextTheme(context, "Roboto", "Libre Baskerville");
    MaterialTheme theme = MaterialTheme(textTheme);

    return MaterialApp(
      title: 'Turbo',
      debugShowCheckedModeBanner: false,
      theme:  theme.light(),
      darkTheme: theme.dark(),
      themeMode: ThemeMode.system,
      // The router handles navigation, including the post-OAuth redirect for web.
      routes: {
        '/': (context) => const MapControllerPage(),
        // This route is hit by the backend redirect for web OAuth flow.
        '/login/success': (context) => const LoginSuccessPage(),
        // This route might be part of an older flow but is kept for compatibility.
        '/login/callback': (context) => const GoogleAuthCallbackPage(),
      },
      initialRoute: '/',
    );
  }
}