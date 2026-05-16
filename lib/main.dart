/// Flutter entry-point shim.
///
/// The real app bootstrap lives in `lib/app/main.dart` per the architecture
/// doc (`lib/context/architecture.context.md` §3). This file exists only so
/// `flutter run` and `flutter build` work without an explicit `-t` flag —
/// Flutter's tooling assumes the entry point is at `lib/main.dart`.
library;

import 'app/main.dart' as app;

void main() => app.main();
