import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_compass/flutter_compass.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Provides the current compass heading in degrees (0–360), or null if unavailable.
final compassStateProvider = StreamProvider.autoDispose<double?>((ref) {
  // Compass is not available on Linux desktop.
  if (!kIsWeb && Platform.isLinux) {
    return const Stream.empty();
  }

  return FlutterCompass.events?.map((event) => event.heading) ??
      const Stream.empty();
});
