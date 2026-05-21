import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as p;

/// Enforces the rule from `lib/context/architecture.context.md`:
///
/// > Each feature exposes a public contract through a single `api.dart` file.
/// > This is the only file that code outside the feature is allowed to import.
///
/// This test walks every Dart file under `lib/features/` and fails if any
/// import crosses a feature boundary without going through that feature's
/// `api.dart`.
///
/// Why it lives in `test/` rather than as a custom_lint rule:
/// no extra tooling to install, runs in normal CI, and the failure message
/// tells the contributor exactly which line to change.
void main() {
  test('no file in lib/features imports another feature except via api.dart',
      () {
    final repoRoot = _findRepoRoot();
    final featuresDir = Directory(p.join(repoRoot, 'lib', 'features'));
    expect(featuresDir.existsSync(), isTrue,
        reason: 'lib/features must exist (cwd=${Directory.current.path})');

    final violations = <_Violation>[];
    for (final entity in featuresDir.listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      violations.addAll(_violationsIn(entity, featuresDir.path));
    }

    if (violations.isEmpty) return;
    final buf = StringBuffer()
      ..writeln('${violations.length} cross-feature boundary '
          'violation(s) found:')
      ..writeln();
    for (final v in violations) {
      buf
        ..writeln('  ${v.importer}:${v.lineNumber}')
        ..writeln('    imports: ${v.imported}')
        ..writeln('    from feature "${v.fromFeature}" → '
            'feature "${v.toFeature}"')
        ..writeln('    fix: import \'package:turbo/features/'
            '${v.toFeature}/api.dart\';')
        ..writeln();
    }
    buf
      ..writeln('See lib/context/architecture.context.md §2 and §4.')
      ..writeln('Each feature is reachable only through its public api.dart.');
    fail(buf.toString());
  });

  test('every features/<X>/api.dart is a pure re-export facade', () {
    final repoRoot = _findRepoRoot();
    final featuresDir = Directory(p.join(repoRoot, 'lib', 'features'));
    final impurities = <_Impurity>[];

    for (final entity in featuresDir.listSync(recursive: true)) {
      if (entity is! File) continue;
      if (p.basename(entity.path) != 'api.dart') continue;
      impurities.addAll(_impuritiesIn(entity, repoRoot));
    }

    if (impurities.isEmpty) return;
    final buf = StringBuffer()
      ..writeln('${impurities.length} api.dart purity violation(s) found.')
      ..writeln('An api.dart must contain only blank lines, comments, '
          '`library;`, and `export ...;` directives.')
      ..writeln('Provider declarations, classes, and `import` directives '
          'belong in data/ files and should be re-exported.')
      ..writeln();
    for (final v in impurities) {
      buf
        ..writeln('  ${v.file}:${v.lineNumber}')
        ..writeln('    ${v.line.trimRight()}')
        ..writeln();
    }
    fail(buf.toString());
  });
}

class _Violation {
  final String importer;
  final int lineNumber;
  final String imported;
  final String fromFeature;
  final String toFeature;

  _Violation({
    required this.importer,
    required this.lineNumber,
    required this.imported,
    required this.fromFeature,
    required this.toFeature,
  });
}

/// `features/<top>/...` — except `features/tile_storage/<sub>/...`, where the
/// feature identity is the leaf (`tile_storage/<sub>`). This matches how the
/// project organizes nested feature modules.
String? _featureOf(String pathRelativeToFeaturesDir) {
  final parts = p.split(pathRelativeToFeaturesDir);
  if (parts.isEmpty) return null;
  if (parts[0] == 'tile_storage' && parts.length >= 2) {
    return 'tile_storage/${parts[1]}';
  }
  return parts[0];
}

/// The `api.dart` file path *relative to `lib/features/`* for the given
/// feature, e.g. `auth/api.dart` or `tile_storage/offline_regions/api.dart`.
String _apiPathFor(String feature) => '$feature/api.dart';

final _importRe = RegExp(r"""^\s*import\s+['"]([^'"]+)['"]""");

Iterable<_Violation> _violationsIn(File dartFile, String featuresDirPath) sync* {
  final importerRel = p.relative(dartFile.path, from: featuresDirPath);
  final importerFeature = _featureOf(importerRel);
  if (importerFeature == null) return;

  final lines = dartFile.readAsLinesSync();
  for (var i = 0; i < lines.length; i++) {
    final match = _importRe.firstMatch(lines[i]);
    if (match == null) continue;
    final raw = match.group(1)!;

    String? targetRelativeToFeatures;
    if (raw.startsWith('package:turbo/features/')) {
      targetRelativeToFeatures =
          raw.substring('package:turbo/features/'.length);
    } else if (raw.startsWith('.')) {
      // Resolve relative to importer's directory; only flag if it lands in
      // lib/features/ at all.
      final absolute = p.normalize(p.join(p.dirname(dartFile.path), raw));
      if (!p.isWithin(featuresDirPath, absolute)) continue;
      targetRelativeToFeatures =
          p.relative(absolute, from: featuresDirPath);
    } else {
      continue;
    }

    final targetFeature = _featureOf(targetRelativeToFeatures);
    if (targetFeature == null || targetFeature == importerFeature) continue;

    if (targetRelativeToFeatures == _apiPathFor(targetFeature)) continue;

    yield _Violation(
      importer: p.relative(dartFile.path, from: p.dirname(featuresDirPath)),
      lineNumber: i + 1,
      imported: targetRelativeToFeatures,
      fromFeature: importerFeature,
      toFeature: targetFeature,
    );
  }
}

class _Impurity {
  final String file;
  final int lineNumber;
  final String line;

  _Impurity({required this.file, required this.lineNumber, required this.line});
}

/// Matches lines that are allowed inside an api.dart facade. Anything else is
/// an impurity:
///   - blank
///   - single-line `//` comment (including dartdoc `///`)
///   - block-comment line (`/* ... */` opener, content, or closer)
///   - `library;` or `library <name>;`
///   - any `export ...;` directive (may span multiple lines for long shows)
final _allowedLineRe = RegExp(
  r'^\s*(' // optional leading whitespace
  r'$' // empty
  r'|//' // line comment
  r'|/\*' // block comment open
  r'|\*' // block comment continuation / close
  r'|library\b' // library directive
  r'|export\b' // export directive
  r')',
);

Iterable<_Impurity> _impuritiesIn(File apiFile, String repoRoot) sync* {
  final lines = apiFile.readAsLinesSync();
  final rel = p.relative(apiFile.path, from: repoRoot);

  // Track whether we're inside a multi-line statement so continuation lines of
  // a long `export ... show A, B, C;` don't get flagged.
  var inExport = false;
  for (var i = 0; i < lines.length; i++) {
    final line = lines[i];
    final trimmed = line.trimLeft();

    if (inExport) {
      if (trimmed.contains(';')) inExport = false;
      continue;
    }
    if (_allowedLineRe.hasMatch(line)) {
      // If this is an export that didn't end on this line, follow it.
      if (trimmed.startsWith('export') && !line.contains(';')) {
        inExport = true;
      }
      continue;
    }
    yield _Impurity(file: rel, lineNumber: i + 1, line: line);
  }
}

/// Walks up from `Directory.current` until it finds a `pubspec.yaml`. The test
/// harness sets `cwd` to the package root in normal `flutter test` runs, but
/// this fallback keeps the test robust to alternative invocations (e.g. an
/// IDE running it from a subdirectory).
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
