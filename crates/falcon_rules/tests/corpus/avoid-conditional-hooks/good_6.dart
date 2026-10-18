import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';

class StableWidget extends HookWidget {
  const StableWidget();

  @override
  Widget build(BuildContext context) {
    final value = useState(0);
    switch (value) {
      default:
        useState(1);
    }
    return const SizedBox();
  }
}

Widget builder() => HookBuilder(builder: (context) {
  useState(0);
  return const SizedBox();
});
