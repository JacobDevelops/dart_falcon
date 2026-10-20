import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
class LocalHooks {
  void useEffect(dynamic effect, List<Object?> keys) {}
  dynamic useMemoized(dynamic builder, List<Object?> keys) => builder();
}
class Example {
  LocalHooks get hooks => LocalHooks();
  void build() {
    hooks.useEffect(() {}, [[1]]);
    hooks.useMemoized(() => 1, [{1}]);
  }
}
