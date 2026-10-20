import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;
void example(int value) {
  useEffect(() {}, [[value]]); /* expect: prefer-use-memo */
  useMemoized(() => value, [{value}]); /* expect: prefer-use-memo */
  useCallback(() {}, [{'value': value}]); /* expect: prefer-use-memo */
  hooks.useMemoized(() => value, [(<int>[value])]); /* expect: prefer-use-memo */
  hooks.useEffect(() {}, [<String, int>{'value': value}]); /* expect: prefer-use-memo */
}
