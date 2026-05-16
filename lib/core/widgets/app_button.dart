import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// The single button primitive for the app. Use one of the named constructors;
/// the underlying styling is enforced by the theme's `filledButtonTheme`,
/// `outlinedButtonTheme`, and `textButtonTheme`.
///
/// `fullWidth` defaults to `false`. Callers that want a full-row CTA (login,
/// "Save Marker", "Save Path") opt in explicitly.
enum _AppButtonKind { primary, secondary, tonal, danger, text }

class AppButton extends StatelessWidget {
  final String text;
  final VoidCallback? onPressed;
  final IconData? icon;
  final bool isLoading;
  final bool fullWidth;
  final EdgeInsetsGeometry? padding;
  final _AppButtonKind _kind;

  const AppButton.primary({
    super.key,
    required this.text,
    this.onPressed,
    this.icon,
    this.isLoading = false,
    this.fullWidth = false,
    this.padding,
  }) : _kind = _AppButtonKind.primary;

  const AppButton.secondary({
    super.key,
    required this.text,
    this.onPressed,
    this.icon,
    this.isLoading = false,
    this.fullWidth = false,
    this.padding,
  }) : _kind = _AppButtonKind.secondary;

  const AppButton.tonal({
    super.key,
    required this.text,
    this.onPressed,
    this.icon,
    this.isLoading = false,
    this.fullWidth = false,
    this.padding,
  }) : _kind = _AppButtonKind.tonal;

  const AppButton.danger({
    super.key,
    required this.text,
    this.onPressed,
    this.icon,
    this.isLoading = false,
    this.fullWidth = false,
    this.padding,
  }) : _kind = _AppButtonKind.danger;

  const AppButton.text({
    super.key,
    required this.text,
    this.onPressed,
    this.icon,
    this.isLoading = false,
    this.fullWidth = false,
    this.padding,
  }) : _kind = _AppButtonKind.text;

  bool get _disabled => isLoading || onPressed == null;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final button = _buildButton(context, colorScheme);
    return fullWidth ? SizedBox(width: double.infinity, child: button) : button;
  }

  Widget _buildButton(BuildContext context, ColorScheme colorScheme) {
    final loader = _LoadingDot(
      color: switch (_kind) {
        _AppButtonKind.primary || _AppButtonKind.danger => colorScheme.onPrimary,
        _AppButtonKind.tonal => colorScheme.onSecondaryContainer,
        _AppButtonKind.secondary || _AppButtonKind.text => colorScheme.primary,
      },
    );
    final label = isLoading ? loader : _label(context);
    switch (_kind) {
      case _AppButtonKind.primary:
        return FilledButton(
          onPressed: _disabled ? null : onPressed,
          style: _stylePadding(),
          child: label,
        );
      case _AppButtonKind.danger:
        return FilledButton(
          onPressed: _disabled ? null : onPressed,
          style: FilledButton.styleFrom(
            backgroundColor: colorScheme.error,
            foregroundColor: colorScheme.onError,
            padding: padding,
          ),
          child: label,
        );
      case _AppButtonKind.tonal:
        return FilledButton.tonal(
          onPressed: _disabled ? null : onPressed,
          style: _stylePadding(),
          child: label,
        );
      case _AppButtonKind.secondary:
        return OutlinedButton(
          onPressed: _disabled ? null : onPressed,
          style: _stylePadding(),
          child: label,
        );
      case _AppButtonKind.text:
        return TextButton(
          onPressed: _disabled ? null : onPressed,
          style: _stylePadding(),
          child: label,
        );
    }
  }

  Widget _label(BuildContext context) {
    if (icon == null) return Text(text);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 20),
        const SizedBox(width: AppSpacing.s),
        Text(text),
      ],
    );
  }

  ButtonStyle? _stylePadding() =>
      padding == null ? null : FilledButton.styleFrom(padding: padding);
}

class _LoadingDot extends StatelessWidget {
  final Color color;
  const _LoadingDot({required this.color});

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 20,
      width: 20,
      child: CircularProgressIndicator(strokeWidth: 2, color: color),
    );
  }
}
