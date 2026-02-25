import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

final connectivityProvider = NotifierProvider<ConnectivityNotifier, bool>(() {
  return ConnectivityNotifier();
});

class ConnectivityNotifier extends Notifier<bool> {
  StreamSubscription<List<ConnectivityResult>>? _subscription;

  @override
  bool build() {
    if (kIsWeb) return true;

    final connectivity = Connectivity();

    // Check current status immediately to avoid assuming online on cold start
    connectivity.checkConnectivity().then((results) {
      state = !results.contains(ConnectivityResult.none);
    });

    _subscription = connectivity.onConnectivityChanged.listen((results) {
      state = !results.contains(ConnectivityResult.none);
    });

    ref.onDispose(() {
      _subscription?.cancel();
    });

    return true;
  }
}
