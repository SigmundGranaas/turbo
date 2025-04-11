import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/widgets/auth/register_modal.dart';
import '../../data/auth/auth_providers.dart';
import 'auth_base_screen.dart';
import 'auth_divider.dart';
import 'auth_error_message.dart';
import 'auth_footer_link.dart';
import 'auth_text_field.dart';
import 'google_sign_in_button.dart';
import 'password_field.dart'; // Optional: Use the new component
import 'primary_button.dart';

class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key});

  static Future<void> show(BuildContext context) {
    return AuthBaseScreen.showResponsive(
      context: context,
      child: const LoginScreen(),
    );
  }

  @override
  ConsumerState<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends ConsumerState<LoginScreen> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();
  bool _isLoading = false;
  bool _isGoogleLoading = false;

  @override
  void initState() {
    super.initState();

    // Listen for auth state changes
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.listenManual(authStateProvider, (previous, next) {
        if (next.status == AuthStatus.authenticated) {
          if (kDebugMode) {
            print("Login successful, closing screen");
          }
          // Close the screen when authenticated
          Navigator.of(context).pop();
        }
      });
    });
  }

  @override
  void dispose() {
    _emailController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  Future<void> _login() async {
    if (_formKey.currentState!.validate()) {
      setState(() {
        _isLoading = true;
      });

      try {
        await ref.read(authStateProvider.notifier).login(
          _emailController.text.trim(),
          _passwordController.text,
        );
      } finally {
        if (mounted) {
          setState(() {
            _isLoading = false;
          });
        }
      }
    }
  }

  void _startGoogleSignIn() {
    setState(() {
      _isGoogleLoading = true;
    });
  }

  void _completeGoogleSignIn() {
    if (mounted) {
      setState(() {
        _isGoogleLoading = false;
      });
    }
  }

  void _navigateToRegister() {
    Navigator.of(context).pop();
    RegisterScreen.show(context);
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.of(context).size;
    final isDesktop = size.width > 768;

    return AuthBaseScreen(
      title: 'Sign in',
      formContent: _buildLoginForm(isDesktop),
      isDesktopView: isDesktop,
    );
  }

  Widget _buildLoginForm(bool isDesktop) {
    final errorMessage = ref.watch(authStateProvider).errorMessage;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          // Error message if any
          if (errorMessage != null) ...[
            AuthErrorMessage(message: errorMessage),
            const SizedBox(height: 24),
          ],

          // Email field
          AuthTextField(
            controller: _emailController,
            label: 'Email',
            hintText: 'Enter your email',
            keyboardType: TextInputType.emailAddress,
            validator: (value) {
              if (value == null || value.isEmpty) {
                return 'Please enter your email';
              }
              if (!RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(value)) {
                return 'Please enter a valid email';
              }
              return null;
            },
          ),
          const SizedBox(height: 24),

          // Using the PasswordField component (optional)
          PasswordField(
            controller: _passwordController,
            label: 'Password',
            validator: (value) {
              if (value == null || value.isEmpty) {
                return 'Please enter your password';
              }
              return null;
            },
          ),

          // Forgot password link
          Align(
            alignment: Alignment.centerRight,
            child: TextButton(
              onPressed: () {
                // Handle forgot password
              },
              style: TextButton.styleFrom(
                foregroundColor: colorScheme.primary,
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 12),
                textStyle: textTheme.labelMedium,
              ),
              child: const Text('Forgot Password?'),
            ),
          ),
          const SizedBox(height: 16),

          // Login button
          PrimaryButton(
            text: 'Sign in',
            onPressed: _login,
            isLoading: _isLoading,
          ),

          const SizedBox(height: 24),

          // Divider
          const AuthDivider(text: 'or'),

          const SizedBox(height: 24),

          // Google sign-in button
          GoogleSignInButton(
            isLoading: _isGoogleLoading,
            onSignInStarted: _startGoogleSignIn,
            onSignInCompleted: _completeGoogleSignIn,
          ),

          const SizedBox(height: 24),

          // Register link
          AuthFooterLink(
            message: 'Don\'t have an account?',
            linkText: 'Create account',
            onPressed: _navigateToRegister,
          ),
        ],
      ),
    );
  }
}