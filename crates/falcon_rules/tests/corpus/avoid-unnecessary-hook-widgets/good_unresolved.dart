import 'package:flutter/widgets.dart';

class UnknownHookWidget extends HookWidget {
  Widget build(BuildContext context) => const SizedBox();
}
final unknownBuilder = HookBuilder(builder: (context) => const SizedBox());
