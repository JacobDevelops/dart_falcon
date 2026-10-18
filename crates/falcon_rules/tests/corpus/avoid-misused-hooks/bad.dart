import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:flutter_hooks/flutter_hooks.dart' as hooks;

int useFeature() => useState(1);
class Screen extends StatelessWidget {
  Widget build(BuildContext context) {
    useState<int>(0).toString(); /* expect: avoid-misused-hooks */
    useMemoized(() => 1); /* expect: avoid-misused-hooks */
    useFeature(); /* expect: avoid-misused-hooks */
    for (var i = 0; i < 2; i++) {
      useState(i); /* expect: avoid-misused-hooks */
    }
    return const SizedBox();
  }
}
abstract class BaseScreen extends StatelessWidget {}
class DerivedScreen extends BaseScreen {
  Widget build(BuildContext context) {
    hooks.useState(0); /* expect: avoid-misused-hooks */
    return const SizedBox();
  }
}
class StatefulScreen extends StatefulWidget {}
class ScreenState extends State<StatefulScreen> {
  Widget build(BuildContext context) {
    hooks.useEffect(() {}); /* expect: avoid-misused-hooks */
    return const SizedBox();
  }
}
