import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
class LocalHooks {
  dynamic useMemoized(dynamic builder) => builder();
}
class Example {
  LocalHooks get hooks => LocalHooks();
  void build() {
    hooks.useMemoized(() => () {});
  }
}
