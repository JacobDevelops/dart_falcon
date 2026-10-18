import 'package:flutter/widgets.dart';
abstract class WidgetRef {}
abstract class ConsumerStatefulWidget extends StatefulWidget {
  const ConsumerStatefulWidget({super.key});
}
abstract class ConsumerState<T extends ConsumerStatefulWidget> extends State<T> {}
abstract class StatefulHookConsumerWidget extends ConsumerStatefulWidget {
  const StatefulHookConsumerWidget({super.key});
}
abstract class HookConsumerWidget extends StatefulHookConsumerWidget {
  const HookConsumerWidget({super.key});
}
class HookConsumer extends HookConsumerWidget {
  const HookConsumer({super.key, required this.builder});
  final Widget Function(BuildContext, WidgetRef, Widget?) builder;
}
