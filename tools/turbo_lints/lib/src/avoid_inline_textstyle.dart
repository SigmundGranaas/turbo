import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/error/listener.dart';
import 'package:custom_lint_builder/custom_lint_builder.dart';

/// Bans the `TextStyle(...)` constructor outside theme code.
///
/// Allowed instead: `Theme.of(context).textTheme.X.copyWith(...)`. A common
/// legitimate use — `TextStyle(color: ...)` to recolor onX text — is OK
/// because we want callers to migrate to AppSnackbars / textTheme copyWith.
class AvoidInlineTextStyle extends DartLintRule {
  const AvoidInlineTextStyle() : super(code: _code);

  static const _code = LintCode(
    name: 'avoid_inline_textstyle',
    problemMessage:
        'Avoid inline TextStyle(...) — use Theme.of(context).textTheme.X.copyWith(...) '
        'so font scale, weight, and color follow the theme.',
  );

  bool _isExempt(String path) {
    return path.contains('/lib/core/theme/') || path.contains('/test/');
  }

  @override
  void run(
    CustomLintResolver resolver,
    ErrorReporter reporter,
    CustomLintContext context,
  ) {
    if (_isExempt(resolver.source.uri.path)) return;

    context.registry.addInstanceCreationExpression((node) {
      final type = node.constructorName.type.element;
      if (type is! ClassElement) return;
      if (type.name != 'TextStyle') return;
      // Skip empty TextStyle() (rare; benign).
      if (node.argumentList.arguments.isEmpty) return;
      reporter.atNode(node, _code);
    });
  }
}
