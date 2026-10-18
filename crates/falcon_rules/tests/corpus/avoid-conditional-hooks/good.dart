import 'package:flutter_hooks/flutter_hooks.dart' as hooks;

int useCounter() => hooks.useState(0);

void useStable(bool enabled, List<int> values) {
  final state = hooks.useState(0);
  hooks.useMemoized(() {
    if (enabled) return 1;
    return 0;
  });
  hooks.useEffect(() {
    for (final value in values) {
      if (value > 0) print(value);
    }
  });
  final condition = hooks.useState(true) && enabled;
  final otherCondition = hooks.useState(false) || enabled;
  final int? optional = hooks.useState<int?>(null);
  final fallback = optional ?? 1;
  final ternary = hooks.useState(true) ? 1 : 0;
  if (hooks.useState(true)) print(state);
  for (var index = hooks.useState(0); index < 2; index++) {
    print(index);
  }
  final custom = useCounter();
  final collection = [if (hooks.useState(true)) custom];
  if (enabled) {
    void callback() {
      hooks.useState(0);
    }
    final callbackExpression = () => hooks.useState(0);
  }
}

class Unrelated {
  int useState(int initial) => initial;

  void useUnrelated(bool enabled) {
    if (enabled) useState(0);
  }
}

void unrelated(bool enabled, Unrelated service) {
  if (enabled) service.useState(0);
}

void useShadowed(bool enabled, int Function(int) useState) {
  if (enabled) useState(0);
}
