import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../data/auth/auth_providers.dart';
import 'auth_base_screen.dart';
import 'auth_divider.dart';
import 'auth_error_message.dart';
import 'auth_footer_link.dart';
import 'auth_text_field.dart';
import 'dev_banner.dart';
import 'google_sign_in_button.dart';
import 'login_modal.dart';
import 'password_field.dart';
import 'primary_button.dart';

class RegisterScreen extends ConsumerStatefulWidget {
  const RegisterScreen({super.key});

  static Future<void> show(BuildContext context) {
    return AuthBaseScreen.showResponsive(
      context: context,
      child: const RegisterScreen(),
    );
  }

  @override
  ConsumerState<RegisterScreen> createState() => _RegisterScreenState();
}

class _RegisterScreenState extends ConsumerState<RegisterScreen> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();
  final _confirmPasswordController = TextEditingController();
  bool _isLoading = false;
  bool _isGoogleLoading = false;
  bool _isNotifyMeLoading = false;

  // Environment check - only show full form in development
  bool get _isDevelopment => true;

  @override
  void initState() {
    super.initState();
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

  // Regular registration function for development mode
  Future<void> _register() async {
    if (_formKey.currentState!.validate()) {
      setState(() {
        _isLoading = true;
      });

      try {
        await ref.read(authStateProvider.notifier).register(
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

  // Waitlist subscription for production mode
  Future<void> _submitNotifyMe() async {
    if (_formKey.currentState!.validate()) {
      setState(() {
        _isNotifyMeLoading = true;
      });

      try {
        // Here you would implement email collection for waitlist
        // For now we'll just simulate a delay
        await Future.delayed(const Duration(seconds: 1));
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(
              content: Text('Thanks! We\'ll notify you when public signups open.'),
              behavior: SnackBarBehavior.floating,
            ),
          );
          _emailController.clear();
        }
      } finally {
        if (mounted) {
          setState(() {
            _isNotifyMeLoading = false;
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
    final size = MediaQuery.of(context).size;
    final isDesktop = size.width > 768;

    return AuthBaseScreen(
      title: _isDevelopment ? 'Create account' : 'Public signups coming soon!',
      subtitle: _isDevelopment
          ? 'To start using Turbo'
          : 'Join our waitlist to be notified when we launch',
      formContent: _isDevelopment
          ? _buildRegistrationForm(isDesktop)
          : _buildComingSoonForm(isDesktop),
      isDesktopView: isDesktop,
    );
  }

  // Development mode: Full registration form
  Widget _buildRegistrationForm(bool isDesktop) {
    final errorMessage = ref.watch(authStateProvider).errorMessage;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          // Development mode notice
          const DevModeBanner(
            message: 'Development mode: Full registration enabled',
          ),
          const SizedBox(height: 24),

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
            hintText: 'Enter your password',
            validator: (value) {
              if (value == null || value.isEmpty) {
                return 'Please enter a password';
              }
              if (value.length < 8) {
                return 'Password must be at least 8 characters';
              }
              return null;
            },
          ),
          const SizedBox(height: 24),

          // Using the PasswordField component (optional)
          PasswordField(
            controller: _confirmPasswordController,
            label: 'Confirm Password',
            hintText: 'Confirm your password',
            validator: (value) {
              if (value == null || value.isEmpty) {
                return 'Please confirm your password';
              }
              if (value != _passwordController.text) {
                return 'Passwords do not match';
              }
              return null;
            },
          ),
          const SizedBox(height: 24),

          // Register button
          PrimaryButton(
            text: 'Create account',
            onPressed: _register,
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

          // Terms of service text
          Text(
            'By creating an account, you agree to our Terms of Service and Privacy Policy',
            textAlign: TextAlign.center,
            style: textTheme.bodySmall?.copyWith(
              color: colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 24),

          // Login link
          AuthFooterLink(
            message: 'Already have an account?',
            linkText: 'Sign in',
            onPressed: _navigateToLogin,
          ),
        ],
      ),
    );
  }

  // Production mode: Coming soon form
  Widget _buildComingSoonForm(bool isDesktop) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        mainAxisSize: MainAxisSize.min,
        children: [
          // Coming soon illustration
          Container(
            padding: const EdgeInsets.all(24),
            decoration: BoxDecoration(
              color: colorScheme.secondaryContainer.withValues(alpha:0.5),
              borderRadius: BorderRadius.circular(20),
            ),
            child: Icon(
              Icons.rocket_launch_rounded,
              size: 80,
              color: colorScheme.secondary,
            ),
          ),
          const SizedBox(height: 32),

          // Explanatory text
          Text(
            'We\'re preparing for launch!',
            style: textTheme.headlineSmall?.copyWith(
              fontWeight: FontWeight.bold,
              color: colorScheme.onSurface,
            ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 16),

          Text(
            'Public registration will be available soon. Join our waitlist to be notified when we launch.',
            style: textTheme.bodyLarge?.copyWith(
              color: colorScheme.onSurfaceVariant,
            ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 32),

          // Email notification form
          AuthTextField(
            controller: _emailController,
            label: 'Email',
            hintText: 'Enter your email for updates',
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

          // Notify me button
          PrimaryButton(
            text: 'Notify me when available',
            onPressed: _submitNotifyMe,
            isLoading: _isNotifyMeLoading,
          ),

          const SizedBox(height: 32),

          // Login link
          AuthFooterLink(
            message: 'Already have access?',
            linkText: 'Sign in',
            onPressed: _navigateToLogin,
          ),
        ],
      ),
    );
  }
}