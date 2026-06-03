import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;

/// Enforces a single bottom-sheet mechanism: every sheet is presented through
/// `showExclusiveSheet` (or `showAppSheet`, which delegates to it), so sheets
/// share one set of params, one drag handle, and a single-at-a-time policy.
///
/// The only place allowed to call Flutter's raw `showModalBottomSheet` is the
/// helper that defines the mechanism — `core/widgets/exclusive_sheet.dart`.
/// Comments and string literals are stripped so doc references don't trip it.
void main() {
  test('no raw showModalBottomSheet outside exclusive_sheet.dart', () {
    final repoRoot = _findRepoRoot();
    final libDir = Directory(p.join(repoRoot, 'lib'));
    expect(libDir.existsSync(), isTrue);

    final allowed = p.join('lib', 'core', 'widgets', 'exclusive_sheet.dart');
    final callRe = RegExp(r'(^|[^A-Za-z0-9_.])showModalBottomSheet\s*[<(]');
    final hits = <String>[];

    for (final entity in libDir.listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      final rel = p.relative(entity.path, from: repoRoot);
      if (rel == allowed) continue;
      final lines = entity.readAsLinesSync();
      for (var i = 0; i < lines.length; i++) {
        final code = _stripLineComment(lines[i]);
        if (callRe.hasMatch(code)) hits.add('$rel:${i + 1}  ${lines[i].trim()}');
      }
    }

    if (hits.isEmpty) return;
    fail('${hits.length} raw showModalBottomSheet call(s) — route them through '
        'showExclusiveSheet/showAppSheet instead:\n  ${hits.join('\n  ')}');
  });
}

String _stripLineComment(String line) {
  var inSingle = false;
  var inDouble = false;
  for (var i = 0; i < line.length; i++) {
    final c = line[i];
    if (c == r'\') {
      i++;
      continue;
    }
    if (!inDouble && c == "'") inSingle = !inSingle;
    if (!inSingle && c == '"') inDouble = !inDouble;
    if (!inSingle &&
        !inDouble &&
        c == '/' &&
        i + 1 < line.length &&
        line[i + 1] == '/') {
      return line.substring(0, i);
    }
  }
  return line;
}

String _findRepoRoot() {
  var dir = Directory.current;
  for (var i = 0; i < 10; i++) {
    if (File(p.join(dir.path, 'pubspec.yaml')).existsSync()) return dir.path;
    final parent = dir.parent;
    if (parent.path == dir.path) break;
    dir = parent;
  }
  return Directory.current.path;
}
