import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;

void example() {
  useCallback(() {});
  useMemoized(() => 42);
  useMemoized(() async => () {});
  useMemoized(() { print('creation'); return () {}; });
  useMemoized(() => existing);
  void local(dynamic useMemoized) { useMemoized(() => () {}); }
  void prefix(dynamic hooks) { hooks.useMemoized(() => () {}); }
}
void existing() {}
class Local {
  dynamic useMemoized(dynamic callback) => callback();
  void build() { useMemoized(() => () {}); }
}
