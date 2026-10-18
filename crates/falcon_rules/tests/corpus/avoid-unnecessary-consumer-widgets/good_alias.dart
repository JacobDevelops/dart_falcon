import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

mixin Marker {}

class AliasedBase extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}

class AliasedConsumer = AliasedBase with Marker;
