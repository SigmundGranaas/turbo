import 'package:flutter_riverpod/flutter_riverpod.dart';

final followModeProvider = NotifierProvider<FollowModeNotifier, bool>(
  FollowModeNotifier.new,
);

class FollowModeNotifier extends Notifier<bool> {
  @override
  bool build() => false;

  void enable() => state = true;
  void disable() => state = false;
  void toggle() => state = !state;
}
