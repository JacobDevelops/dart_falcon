import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;

void example() {
  useMemoized(() => () {}); /* expect: prefer-use-callback */
  hooks.useMemoized(() => (int value) => value + 1); /* expect: prefer-use-callback */
  useMemoized(() { return () {}; }); /* expect: prefer-use-callback */
  useMemoized<void Function()>(() => () {}); /* expect: prefer-use-callback */
  useMemoized(() => (() {}), [1]); /* expect: prefer-use-callback */
}
