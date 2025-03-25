import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../data/auth/auth_providers.dart';

class LoginSuccessPage extends ConsumerStatefulWidget {
  const LoginSuccessPage({super.key});

  @override
  ConsumerState<LoginSuccessPage> createState() => _LoginSuccessPageState();
}

class _LoginSuccessPageState extends ConsumerState<LoginSuccessPage> {
  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final authState = ref.watch(authStateProvider);

    return Scaffold(
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(
              Icons.check_circle_outline,
              color: Colors.green,
              size: 80,
            ),
            const SizedBox(height: 24),
            Text(
              'Login Successful!',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 16),
            Text(
              authState.status == AuthStatus.authenticated
                  ? 'Welcome back, ${authState.email}'
                  : 'Redirecting to app...',
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 24),
            const CircularProgressIndicator(),
          ],
        ),
      ),
    );
  }
}