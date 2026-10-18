import 'package:flutter_hooks/flutter_hooks.dart';

void useShadowed(bool enabled, int Function(int) useState) {
  if (enabled) useState(0);
}

class Local {
  int useState(int initial) => initial;

  void useUnrelated(bool enabled) {
    if (enabled) useState(0);
  }
}
