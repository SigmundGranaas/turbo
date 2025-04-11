import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_fonts/google_fonts.dart';

/// Base screen for all authentication-related screens (login, register, etc.)
/// Handles common layout for both mobile and desktop views
class AuthBaseScreen extends ConsumerWidget {
  final String title;
  final String? subtitle;
  final Widget formContent;
  final Color? headerBackgroundColor;
  final bool isDesktopView;

  const AuthBaseScreen({
    super.key,
    required this.title,
    this.subtitle,
    required this.formContent,
    this.headerBackgroundColor,
    required this.isDesktopView,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final viewInsets = MediaQuery.of(context).viewInsets;

    Widget content = Card(
      margin: EdgeInsets.symmetric(
        horizontal: isDesktopView ? 0 : 16,
        vertical: isDesktopView ? 24 : 0,
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
              margin: EdgeInsets.fromLTRB(24, isDesktopView ? 36 : 0, 24, 24),
              decoration: BoxDecoration(
                color: headerBackgroundColor ?? colorScheme.primaryContainer,
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

                  // Screen title
                  Text(
                    title,
                    style: textTheme.titleLarge?.copyWith(
                      color: colorScheme.onPrimaryContainer,
                    ),
                  ),

                  // Optional subtitle
                  if (subtitle != null)
                    Text(
                      subtitle!,
                      style: textTheme.bodyMedium?.copyWith(
                        color: colorScheme.onPrimaryContainer,
                      ),
                    ),
                ],
              ),
            ),

            // Form content area with padding
            Padding(
              padding: EdgeInsets.symmetric(
                horizontal: isDesktopView ? 32 : 24,
              ),
              child: formContent,
            ),
          ],
        ),
      ),
    );

    // For desktop, return just the card content for the dialog
    if (isDesktopView) {
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

  /// Static helper for responsive display - either as dialog or full screen
  static Future<void> showResponsive({
    required BuildContext context,
    required Widget child,
  }) {
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
                child: child,
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
          builder: (context) => child,
        ),
      );
    }
  }
}