import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
void example(int value) {
  useEffect(() {}, [value]);
  useMemoized(() => value, [const [1]]);
  useCallback(() {}, [const {1}]);
  useEffect(() {}, [const {'value': 1}]);
  useMemoized(() => value, const [[1], {2}, {'value': 3}]);
  final memo = useMemoized(() => [value], [value]);
  useEffect(() {}, [memo]);
  void shadow(dynamic useEffect) { useEffect(() {}, [[value]]); }
  void prefix(dynamic hooks) { hooks.useEffect(() {}, [[value]]); }
}
class Local {
  dynamic useEffect(dynamic fn, dynamic keys) {}
  void example() { useEffect(() {}, [[1]]); }
}
