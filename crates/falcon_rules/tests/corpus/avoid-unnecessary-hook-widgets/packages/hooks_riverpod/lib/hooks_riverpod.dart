import 'package:flutter/widgets.dart';

class WidgetRef {}
abstract class HookConsumerWidget extends Widget {
  const HookConsumerWidget({super.key});
  Widget build(BuildContext context, WidgetRef ref);
}
