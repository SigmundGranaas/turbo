import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show ProviderListenable;

const _defaultTimeout = Duration(seconds: 2);
const _defaultPollInterval = Duration(milliseconds: 20);

/// Polls [provider] (an `AsyncValue<T>` returning provider) until it settles
/// into `AsyncData`, then returns the value. Throws on `AsyncError`. Times out
/// with a descriptive `TimeoutException` after [timeout].
///
/// Replaces the per-file `_waitForData` poller from
/// `path_customization_e2e_test.dart` and `marker_behavior_test.dart`.
Future<T> waitForAsyncData<T>(
  ProviderContainer container,
  ProviderListenable<AsyncValue<T>> provider, {
  Duration timeout = _defaultTimeout,
  Duration pollInterval = _defaultPollInterval,
}) async {
  final iterations = timeout.inMilliseconds ~/ pollInterval.inMilliseconds;
  for (var i = 0; i < iterations; i++) {
    await Future.delayed(pollInterval);
    final s = container.read(provider);
    if (s is AsyncData<T>) return s.value;
    if (s is AsyncError) throw (s as AsyncError).error;
  }
  throw TimeoutException(
      'Provider did not settle into AsyncData within $timeout');
}

/// Polls any provider until [predicate] returns true. Useful for plain
/// (non-Async) Notifier states like `tileRegistryProvider` after a
/// `SharedPreferences` rehydrate.
Future<T> waitForState<T>(
  ProviderContainer container,
  ProviderListenable<T> provider,
  bool Function(T) predicate, {
  Duration timeout = _defaultTimeout,
  Duration pollInterval = _defaultPollInterval,
}) async {
  final iterations = timeout.inMilliseconds ~/ pollInterval.inMilliseconds;
  for (var i = 0; i < iterations; i++) {
    final value = container.read(provider);
    if (predicate(value)) return value;
    await Future.delayed(pollInterval);
  }
  throw TimeoutException(
      'Provider state did not satisfy predicate within $timeout');
}
