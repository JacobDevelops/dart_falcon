import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;

void useStable(bool enabled) {
  hooks.useState(0);
  hooks.useMemoized(() => enabled ? 1 : 0);
  if (enabled) {
    final callback = () {
      hooks.useState(0);
    };
  }
}
