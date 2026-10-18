import 'package:flutter_hooks/flutter_hooks.dart';

class CustomHooks {
  int useCounter() => useState(0);

  void useConditional(bool enabled) {
    if (enabled) {
      useCounter(); /* expect: avoid-conditional-hooks */
      useState(0).toString(); /* expect: avoid-conditional-hooks */
      useMemoized(() => () => 1)(); /* expect: avoid-conditional-hooks */
    }
    switch (enabled) {
      case true:
        break;
      default:
        useState(0); /* expect: avoid-conditional-hooks */
    }
    final selected = switch (enabled) {
      true when useState(true) => 1, /* expect: avoid-conditional-hooks */
      _ => 0,
    };
  }
}
