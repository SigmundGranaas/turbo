import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/theme.dart';
import 'package:map_app/utils.dart';
import 'package:map_app/widgets/auth/google_oauth_screen.dart';
import 'package:map_app/widgets/auth/login_success.dart';
import 'package:map_app/widgets/map/main_map.dart';

import 'data/datastore/factory.dart';
import 'data/state/providers/initialize_tiles_provider.dart';
import 'data/auth/auth_providers.dart';

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

    final brightness = View.of(context).platformDispatcher.platformBrightness;
    // Create custom text theme
    TextTheme textTheme = createTextTheme(context, "Roboto", "Libre Baskerville");
    MaterialTheme theme = MaterialTheme(textTheme);

    return MaterialApp(
      title: 'Turbo',
      debugShowCheckedModeBanner: false,
      theme:  theme.light(),
      // Define routes for navigation
      routes: {
        '/': (context) => const HomeWrapper(),
        '/login/callback': (context) => const GoogleAuthCallbackPage(),
        '/login/success': (context) => const LoginSuccessPage(),
      },
      initialRoute: '/',
    );
  }
}

class HomeWrapper extends ConsumerStatefulWidget {
  const HomeWrapper({super.key});

  @override
  ConsumerState<HomeWrapper> createState() => _HomeWrapperState();
}

class _HomeWrapperState extends ConsumerState<HomeWrapper> {
  bool _isInitializing = true;

  @override
  void initState() {
    super.initState();

    // Initialize auth silently without blocking the app
    _initializeAuth();
  }

  Future<void> _initializeAuth() async {
    try {
      await ref.read(authStateProvider.notifier).initialize();
    } catch (e) {
      if (kDebugMode) {
        print('Auth initialization error: $e');
      }
    } finally {
      if (mounted) {
        setState(() {
          _isInitializing = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return const MapControllerPage();
  }
}
