import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/error/listener.dart';
import 'package:custom_lint_builder/custom_lint_builder.dart';

/// Bans `Colors.X` (except `Colors.transparent`) and `Color(0x…)` constructors
/// outside the theme layer. Path allow-list: theme/, dev_banner.dart,
/// color_circle.dart (WCAG contrast), saved_paths/models/path_style.dart
/// (intentional palette).
class AvoidHardcodedColors extends DartLintRule {
  const AvoidHardcodedColors() : super(code: _code);

  static const _code = LintCode(
    name: 'avoid_hardcoded_colors',
    problemMessage:
        'Hardcoded colors are not allowed outside lib/core/theme. Use '
        'Theme.of(context).colorScheme or a token from theme/.',
  );

  bool _isExempt(String path) {
    return path.contains('/lib/core/theme/') ||
        path.contains('/test/') ||
        path.endsWith('/lib/features/auth/widgets/dev_banner.dart') ||
        path.endsWith('/lib/core/widgets/color_circle.dart') ||
        path.endsWith('/lib/features/saved_paths/models/path_style.dart');
  }

  @override
  void run(
    CustomLintResolver resolver,
    ErrorReporter reporter,
    CustomLintContext context,
  ) {
    final filePath = resolver.source.uri.path;
    if (_isExempt(filePath)) return;

    context.registry.addPrefixedIdentifier((node) {
      final prefix = node.prefix.name;
      final ident = node.identifier.name;
      if (prefix == 'Colors' && ident != 'transparent') {
        reporter.atNode(node, _code);
      }
    });

    context.registry.addPropertyAccess((node) {
      // catches Colors.green.shade100 etc.
      final target = node.target?.toSource();
      if (target == null) return;
      if (target.startsWith('Colors.') && !target.startsWith('Colors.transparent')) {
        reporter.atNode(node, _code);
      }
    });

    context.registry.addInstanceCreationExpression((node) {
      final type = node.constructorName.type.element;
      if (type is! ClassElement) return;
      if (type.name != 'Color') return;
      // Allow Color() if all arguments are non-literals (e.g. computed) — only
      // flag the hex-literal form Color(0xFFAABBCC).
      final args = node.argumentList.arguments;
      if (args.length == 1) {
        final arg = args.first.toSource();
        if (arg.startsWith('0x')) {
          reporter.atNode(node, _code);
        }
      }
    });
  }
}
