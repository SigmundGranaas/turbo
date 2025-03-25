import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:map_app/widgets/auth/register_modal.dart';
import '../../data/auth/auth_providers.dart';
import 'auth_error_message.dart';
import 'auth_text_field.dart';
import 'google_sign_in_button.dart';
import 'primary_button.dart';

class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key});

  // Method to show the screen - as a modal on desktop, full screen on mobile
  static Future<void> show(BuildContext context) {
    final isDesktop = MediaQuery.of(context).size.width > 768;


      // Show as modal dialog on desktop
      return showDialog(
        context: context,
        barrierDismissible: true,
        builder: (BuildContext context) {
          return Dialog(
            backgroundColor: Colors.transparent,
            elevation: 0,
            insetPadding: const EdgeInsets.all(16),
            child: SizedBox(
              width: 500,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(28),
                child: const LoginScreen(),
              ),
            ),
          );
        },
      );
  }

  @override
  ConsumerState<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends ConsumerState<LoginScreen> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();
  bool _obscurePassword = true;
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
    final errorMessage = ref.watch(authStateProvider).errorMessage;
    final size = MediaQuery.of(context).size;
    final isDesktop = size.width > 768;
    final isMobile = !isDesktop;

    // Get theme colors from the app's theme
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Card(
                  margin: EdgeInsets.symmetric(
                    horizontal: isDesktop ? 0 : 16,
                    vertical: 24,
                  ),
                  elevation: 0,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(28),
                  ),
                  color: colorScheme.surface,
                  child: SizedBox(
                    width: isDesktop ? 500 : double.infinity,
                    child: Padding(
                      padding: const EdgeInsets.only(bottom: 24),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          // Header section with app branding
                          Container(
                            width: double.infinity,
                            margin: const EdgeInsets.fromLTRB(24, 36, 24, 24),
                            decoration: BoxDecoration(
                              color: colorScheme.primaryContainer,
                              borderRadius: BorderRadius.circular(24),
                            ),
                            padding: const EdgeInsets.fromLTRB(24, 32, 24, 24),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                // Logo/App name
                                Text(
                                  'Turbo',
                                  style: GoogleFonts.libreBaskerville(
                                    fontSize: 36,
                                    fontWeight: FontWeight.bold,
                                    color: colorScheme.onPrimaryContainer,
                                  ),
                                ),
                                const SizedBox(height: 24),

                                // Welcome text
                                Text(
                                  'Sign in',
                                  style: textTheme.titleLarge?.copyWith(
                                    color: colorScheme.onPrimaryContainer,
                                  ),
                                ),
                                Text(
                                  'To get started',
                                  style: textTheme.bodyMedium?.copyWith(
                                    color: colorScheme.onPrimaryContainer,
                                  ),
                                ),
                              ],
                            ),
                          ),

                          // Form content
                          Padding(
                            padding: EdgeInsets.symmetric(
                              horizontal: isDesktop ? 32 : 24,
                            ),
                            child: _buildLoginForm(errorMessage, isDesktop),
                          ),
                        ],
                      ),
                    ),
                  ),
                );
  }

  Widget _buildLoginForm(String? errorMessage, bool isDesktop) {
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

          // Email field - using theme colors
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

          // Password field - using theme colors
          AuthTextField(
            controller: _passwordController,
            label: 'Password',
            obscureText: _obscurePassword,
            validator: (value) {
              if (value == null || value.isEmpty) {
                return 'Please enter your password';
              }
              return null;
            },
            suffixIcon: IconButton(
              icon: Icon(
                _obscurePassword ? Icons.visibility_outlined : Icons.visibility_off_outlined,
                color: colorScheme.onSurfaceVariant,
                size: 20,
              ),
              onPressed: () {
                setState(() {
                  _obscurePassword = !_obscurePassword;
                });
              },
            ),
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

          // Login button - using theme primary color
          PrimaryButton(
            text: 'Sign in',
            onPressed: _login,
            isLoading: _isLoading,
          ),

          const SizedBox(height: 24),

          Row(
            children: [
              Expanded(child: Divider(color: colorScheme.outline.withOpacity(0.5))),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Text(
                  'or',
                  style: textTheme.bodySmall?.copyWith(
                    color: colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              Expanded(child: Divider(color: colorScheme.outline.withOpacity(0.5))),
            ],
          ),
          const SizedBox(height: 24),

          // Google sign-in button
          GoogleSignInButton(
            isLoading: _isGoogleLoading,
            onSignInStarted: _startGoogleSignIn,
            onSignInCompleted: _completeGoogleSignIn,
          ),

          const SizedBox(height: 24),

          // Create account option
          Center(
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  'Don\'t have an account?',
                  style: textTheme.bodySmall?.copyWith(
                    color: colorScheme.onSurfaceVariant,
                  ),
                ),
                TextButton(
                  onPressed: _navigateToRegister,
                  style: TextButton.styleFrom(
                    foregroundColor: colorScheme.primary,
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
                    textStyle: textTheme.labelMedium,
                  ),
                  child: const Text('Create account'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}