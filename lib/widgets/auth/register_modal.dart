import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';
import '../../data/auth/auth_providers.dart';
import 'auth_error_message.dart';
import 'auth_text_field.dart';
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
  bool _isNotifyMeLoading = false;

  // Environment check - only show full form in development
  bool get _isDevelopment => kDebugMode;

  @override
  void dispose() {
    _emailController.dispose();
    super.dispose();
  }

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
        }
      } finally {
        if (mounted) {
          setState(() {
            _isNotifyMeLoading = false;
            _emailController.clear();
          });
        }
      }
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

                  // Show different text based on environment
                  Text(
                    _isDevelopment ? 'Create account' : 'Public signups coming soon!',
                    style: textTheme.titleLarge?.copyWith(
                      color: colorScheme.onPrimaryContainer,
                    ),
                  ),
                  Text(
                    _isDevelopment
                        ? 'To start using Turbo'
                        : 'Join our waitlist to be notified when we launch',
                    style: textTheme.bodyMedium?.copyWith(
                      color: colorScheme.onPrimaryContainer,
                    ),
                  ),
                ],
              ),
            ),

            // Form content - switch based on environment
            Padding(
              padding: EdgeInsets.symmetric(
                horizontal: isDesktop ? 32 : 24,
              ),
              child: _isDevelopment
                  ? _buildRegisterForm(isDesktop)
                  : _buildComingSoonContent(isDesktop),
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
      backgroundColor: colorScheme.surface,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        leading: IconButton(
          icon: Icon(Icons.close, color: colorScheme.onSurface),
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

  // This method builds the full registration form for development mode
  Widget _buildRegisterForm(bool isDesktop) {
    // Access the existing auth error message
    final errorMessage = ref.watch(authStateProvider).errorMessage;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          // Show debugging notice
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: Colors.amber.withValues(alpha: 0.2),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: Colors.amber),
            ),
            child: Row(
              children: [
                Icon(Icons.info_outline, size: 20, color: Colors.amber.shade800),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    'Development mode: Full registration enabled',
                    style: textTheme.bodyMedium?.copyWith(
                      color: Colors.amber.shade800,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),

          // Error message if any
          if (errorMessage != null) ...[
            AuthErrorMessage(message: errorMessage),
            const SizedBox(height: 24),
          ],

          // Include the rest of your original registration form here...
          // ...

          // This is a placeholder for the original registration form fields
          Text(
            'Full registration form is available in development mode',
            style: textTheme.bodyMedium,
            textAlign: TextAlign.center,
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

  // This method builds the coming soon content for production
  Widget _buildComingSoonContent(bool isDesktop) {
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
              color: colorScheme.secondaryContainer.withValues(alpha: 0.5),
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

          // Login option for existing users
          Center(
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  'Already have access?',
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