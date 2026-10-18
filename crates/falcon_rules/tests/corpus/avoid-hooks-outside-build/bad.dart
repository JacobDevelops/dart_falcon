import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;

final initial = useState<int>(0).toString(); /* expect: avoid-hooks-outside-build */
void start() {
  hooks.useEffect(() {}); /* expect: avoid-hooks-outside-build */
}
int useComposed() => useState(0);
void callCustom() {
  useComposed(); /* expect: avoid-hooks-outside-build */
}
class Screen extends HookWidget {
  void onPressed() {
    useMemoized(() => 1); /* expect: avoid-hooks-outside-build */
  }
  Widget build(BuildContext context) {
    final callback = () {
      useState(1); /* expect: avoid-hooks-outside-build */
    };
    return HookBuilder(builder: (context) {
      final nested = () {
        useState(2); /* expect: avoid-hooks-outside-build */
      };
      return const SizedBox();
    });
  }
}
void _useFeature() {
  final callback = () {
    useState(3); /* expect: avoid-hooks-outside-build */
  };
}
class Service {
  void build() {
    useState(4); /* expect: avoid-hooks-outside-build */
  }
}
