import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/error/listener.dart';
import 'package:custom_lint_builder/custom_lint_builder.dart';

/// Enforces the project radius scale: {8, 12, 16, 28, 100}.
///
/// Catches `BorderRadius.circular(N)` and `Radius.circular(N)` where N is a
/// literal not in the scale. The drag-handle 2dp is intentionally allowed
/// inside `app_sheet.dart` and `map_layer_button.dart` (the only two valid
/// callsites for that value).
class AllowedBorderRadius extends DartLintRule {
  const AllowedBorderRadius() : super(code: _code);

  static const _allowed = {8, 12, 16, 28, 100};

  static const _code = LintCode(
    name: 'allowed_border_radius',
    problemMessage:
        'Border radius must use the project scale: AppRadius.s (8), .m (12), '
        '.l (16), .xl (28), or .pill (100). Add a new value to '
        'lib/core/theme/tokens.dart if a new size is genuinely needed.',
  );

  bool _isExempt(String path) {
    return path.contains('/lib/core/theme/') ||
        path.contains('/test/') ||
        path.endsWith('/lib/core/widgets/app_sheet.dart') ||
        path.endsWith('/lib/features/map_view/widgets/buttons/map_layer_button.dart');
  }

  @override
  void run(
    CustomLintResolver resolver,
    ErrorReporter reporter,
    CustomLintContext context,
  ) {
    if (_isExempt(resolver.source.uri.path)) return;

    context.registry.addMethodInvocation((node) {
      final name = node.methodName.name;
      if (name != 'circular') return;
      final target = node.target?.toSource();
      if (target != 'BorderRadius' && target != 'Radius') return;
      final args = node.argumentList.arguments;
      if (args.length != 1) return;
      final arg = args.first;
      // Only flag literal numeric values; dynamic expressions are allowed.
      if (arg is! IntegerLiteral && arg is! DoubleLiteral) return;
      final value = arg is IntegerLiteral
          ? arg.value
          : (arg as DoubleLiteral).value.toInt();
      if (value == null) return;
      if (_allowed.contains(value)) return;
      reporter.atNode(node, _code);
    });
  }
}
