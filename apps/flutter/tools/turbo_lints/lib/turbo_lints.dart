import 'package:custom_lint_builder/custom_lint_builder.dart';

import 'src/allowed_border_radius.dart';
import 'src/avoid_hardcoded_colors.dart';
import 'src/avoid_inline_textstyle.dart';

PluginBase createPlugin() => _TurboLints();

class _TurboLints extends PluginBase {
  @override
  List<LintRule> getLintRules(CustomLintConfigs configs) => const [
        AvoidHardcodedColors(),
        AvoidInlineTextStyle(),
        AllowedBorderRadius(),
      ];
}
