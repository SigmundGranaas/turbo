import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;

/// Enforces that no feature code emits unguarded `print(...)` or
/// `debugPrint(...)` calls. Logging goes through `package:logging`'s `Logger`
/// (see `lib/core/service/logger.dart`), which `setupLogging()` gates behind
/// `kDebugMode` for the print-to-console fallback.
///
/// `print` literally embedded in a string (e.g. an error message that mentions
/// "print") is ignored — the regex only matches call sites: an identifier
/// boundary, `print(`/`debugPrint(`, anywhere not preceded by `.` or
/// identifier characters (so `someObj.print(...)` and `sprint(...)` don't
/// trigger).
void main() {
  test('no print/debugPrint call sites in lib/features/', () {
    final repoRoot = _findRepoRoot();
    final featuresDir = Directory(p.join(repoRoot, 'lib', 'features'));
    expect(featuresDir.existsSync(), isTrue,
        reason: 'lib/features must exist (cwd=${Directory.current.path})');

    final hits = <_Hit>[];
    for (final entity in featuresDir.listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      hits.addAll(_hitsIn(entity, repoRoot));
    }

    if (hits.isEmpty) return;
    final buf = StringBuffer()
      ..writeln('${hits.length} unguarded print/debugPrint call(s) in '
          'lib/features/:')
      ..writeln();
    for (final h in hits) {
      buf
        ..writeln('  ${h.file}:${h.lineNumber}')
        ..writeln('    ${h.line.trimRight()}')
        ..writeln();
    }
    buf.writeln('Use the package:logging Logger instead (see '
        'lib/core/service/logger.dart).');
    fail(buf.toString());
  });
}

class _Hit {
  final String file;
  final int lineNumber;
  final String line;
  _Hit({required this.file, required this.lineNumber, required this.line});
}

// Matches `print(` or `debugPrint(` as a call. Negative look-behind via
// alternation: the previous character (if any) must NOT be an identifier
// character or a `.`. We strip comments first so commented-out examples or
// `///` references don't match.
final _printCallRe = RegExp(r'(^|[^A-Za-z0-9_.])(?:print|debugPrint)\s*\(');

Iterable<_Hit> _hitsIn(File dartFile, String repoRoot) sync* {
  final rel = p.relative(dartFile.path, from: repoRoot);
  final lines = dartFile.readAsLinesSync();
  for (var i = 0; i < lines.length; i++) {
    final line = lines[i];
    final code = _stripLineComment(line);
    if (code.isEmpty) continue;
    if (_printCallRe.hasMatch(code)) {
      yield _Hit(file: rel, lineNumber: i + 1, line: line);
    }
  }
}

/// Removes a `//`-style line comment from a single line, but leaves `//`
/// occurrences inside string literals alone. Cheap state machine: we don't
/// need full Dart parsing — only enough to avoid false positives on lines
/// like `final s = "see //comment";`.
String _stripLineComment(String line) {
  var inSingle = false;
  var inDouble = false;
  for (var i = 0; i < line.length; i++) {
    final c = line[i];
    if (c == r'\\') {
      i++;
      continue;
    }
    if (!inDouble && c == "'") inSingle = !inSingle;
    if (!inSingle && c == '"') inDouble = !inDouble;
    if (!inSingle && !inDouble && c == '/' && i + 1 < line.length &&
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
