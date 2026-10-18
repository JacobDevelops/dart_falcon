import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Screen extends ConsumerWidget {
  void action(WidgetRef ref) { ref.watch(countProvider); /* expect: avoid-ref-watch-outside-build */ }
  int getCount(WidgetRef reader) => reader.watch(countProvider); /* expect: avoid-ref-watch-outside-build */
  Object build(Object context, WidgetRef ref) {
    final callback = () => ref.watch(countProvider); /* expect: avoid-ref-watch-outside-build */
    void local() { ref.watch(countProvider); } /* expect: avoid-ref-watch-outside-build */
    return callback;
  }
}
class ScreenState extends ConsumerState<Object> {
  void initState() { ref.watch(countProvider); } /* expect: avoid-ref-watch-outside-build */
  Object build(Object context) => 0;
}
void action(WidgetRef reader) { reader.watch(countProvider); } /* expect: avoid-ref-watch-outside-build */
class HookScreen extends hooks.HookConsumerWidget {
  Object build(Object context, WidgetRef ref) => 0;
  void action(hooks.WidgetRef ref) { ref.watch(countProvider); } /* expect: avoid-ref-watch-outside-build */
}
class MenuScreen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) =>
      PopupMenuButton(itemBuilder: (context) => ref.watch(countProvider)); /* expect: avoid-ref-watch-outside-build */
}
