import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import '../../data/auth/auth_providers.dart';
import 'auth_error_message.dart';
import 'auth_text_field.dart';
import 'google_sign_in_button.dart';
import 'login_modal.dart';
import 'primary_button.dart';

class RegisterScreen extends ConsumerStatefulWidget {
  const RegisterScreen({super.key});

  static Future<void> show(BuildContext context) {
    final size = MediaQuery.of(context).size;
    final isDesktop = size.width > 768;

    if (isDesktop) {
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
                child: const RegisterScreen(),
              ),
            ),
          );
        },
      );
    } else {
      // Show as full page on mobile
      return Navigator.of(context).push(
        MaterialPageRoute(
          fullscreenDialog: true,
          builder: (context) => const RegisterScreen(),
        ),
      );
    }
  }

  @override
  ConsumerState<RegisterScreen> createState() => _RegisterScreenState();
}

class _RegisterScreenState extends ConsumerState<RegisterScreen> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();
  final _confirmPasswordController = TextEditingController();
  bool _obscurePassword = true;
  bool _obscureConfirmPassword = true;
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
            print("Registration successful, closing screen");
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
    _confirmPasswordController.dispose();
    super.dispose();
  }

  Future<void> _register() async {
    if (_formKey.currentState!.validate()) {
      setState(() {
        _isLoading = true;
      });

      try {
        await ref.read(authStateProvider.notifier).register(
            _emailController.text.trim(),
            _passwordController.text
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

  void _navigateToLogin() {
    Navigator.of(context).pop();
    LoginScreen.show(context);
  }

  @override
  Widget build(BuildContext context) {
    final errorMessage = ref.watch(authStateProvider).errorMessage;
    final size = MediaQuery.of(context).size;
    final isDesktop = size.width > 768;
    final viewInsets = MediaQuery.of(context).viewInsets;

    // Get theme colors from the app's theme
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    Widget content = Card(
      margin: EdgeInsets.symmetric(
        horizontal: isDesktop ? 0 : 16,
        vertical: isDesktop ? 24 : 0,
      ),
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(28),
      ),
      color: colorScheme.surface,
      child: SingleChildScrollView(
        physics: const ClampingScrollPhysics(),
        padding: EdgeInsets.only(
          bottom: viewInsets.bottom > 0 ? viewInsets.bottom : 24,
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Header section with app branding
            Container(
              width: double.infinity,
              margin: EdgeInsets.fromLTRB(24, isDesktop ? 36 : 0, 24, 24),
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
                    'Create account',
                    style: textTheme.titleLarge?.copyWith(
                      color: colorScheme.onPrimaryContainer,
                    ),
                  ),
                  Text(
                    'To start using Turbo',
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
              child: _buildRegisterForm(errorMessage, isDesktop),
            ),
          ],
        ),
      ),
    );

    // For desktop, return just the card content for the dialog
    if (isDesktop) {
      return content;
    }

    // For mobile, wrap in a Scaffold for full page view
    return Scaffold(
      backgroundColor: colorScheme.background,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        leading: IconButton(
          icon: Icon(Icons.close, color: colorScheme.onBackground),
          onPressed: () => Navigator.of(context).pop(),
        ),
      ),
      body: SafeArea(
        minimum: const EdgeInsets.symmetric(horizontal: 16),
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 500),
            child: content,
          ),
        ),
      ),
    );
  }

  Widget _buildRegisterForm(String? errorMessage, bool isDesktop) {
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
                return 'Please enter a password';
              }
              if (value.length < 8) {
                return 'Password must be at least 8 characters';
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
          const SizedBox(height: 24),

          // Confirm Password field - using theme colors
          AuthTextField(
            controller: _confirmPasswordController,
            label: 'Confirm Password',
            obscureText: _obscureConfirmPassword,
            validator: (value) {
              if (value == null || value.isEmpty) {
                return 'Please confirm your password';
              }
              if (value != _passwordController.text) {
                return 'Passwords do not match';
              }
              return null;
            },
            suffixIcon: IconButton(
              icon: Icon(
                _obscureConfirmPassword ? Icons.visibility_outlined : Icons.visibility_off_outlined,
                color: colorScheme.onSurfaceVariant,
                size: 20,
              ),
              onPressed: () {
                setState(() {
                  _obscureConfirmPassword = !_obscureConfirmPassword;
                });
              },
            ),
          ),
          const SizedBox(height:
          24),

          // Register button - using theme primary color
          PrimaryButton(
            text: 'Create account',
            onPressed: _register,
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

          // Terms of service text
          Text(
            'By creating an account, you agree to our Terms of Service and Privacy Policy',
            textAlign: TextAlign.center,
            style: textTheme.bodySmall?.copyWith(
              color: colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 24),

          // Login option
          Center(
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  'Already have an account?',
                  style: textTheme.bodySmall?.copyWith(
                    color: colorScheme.onSurfaceVariant,
                  ),
                ),
                TextButton(
                  onPressed: _navigateToLogin,
                  style: TextButton.styleFrom(
                    foregroundColor: colorScheme.primary,
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
                    textStyle: textTheme.labelMedium,
                  ),
                  child: const Text('Sign in'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}