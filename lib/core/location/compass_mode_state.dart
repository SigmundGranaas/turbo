import 'package:flutter_riverpod/flutter_riverpod.dart';

final compassModeProvider = NotifierProvider<CompassModeNotifier, bool>(
  CompassModeNotifier.new,
);

class CompassModeNotifier extends Notifier<bool> {
  @override
  bool build() => false;

  void enable() => state = true;
  void disable() => state = false;
  void toggle() => state = !state;
}
