import 'package:flutter/widgets.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as riverpod;

class ConditionalWidget extends HookWidget {
  final bool enabled;
  const ConditionalWidget(this.enabled);

  @override
  Widget build(BuildContext context) {
    if (enabled) useState(0); /* expect: avoid-conditional-hooks */
    return const SizedBox();
  }
}

class ConditionalConsumer extends riverpod.HookConsumerWidget {
  final bool enabled;
  const ConditionalConsumer(this.enabled);

  @override
  Widget build(BuildContext context) {
    if (enabled) useState(0); /* expect: avoid-conditional-hooks */
    return const SizedBox();
  }
}

Widget builder(bool enabled) => HookBuilder(builder: (context) {
  if (enabled) useState(0); /* expect: avoid-conditional-hooks */
  return const SizedBox();
});
