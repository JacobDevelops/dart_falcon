import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Screen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    final value = ref.read(countProvider); /* expect: avoid-ref-read-inside-build */
    if (value > 0) { return ref.read(countProvider); } /* expect: avoid-ref-read-inside-build */
    return ref.read<int>(countProvider); /* expect: avoid-ref-read-inside-build */
  }
}
class Arrow extends ConsumerWidget {
  Object build(Object context, WidgetRef reader) => reader.read(countProvider); /* expect: avoid-ref-read-inside-build */
}
class ScreenState extends ConsumerState<Object> {
  Object build(Object context) => ref.read(countProvider); /* expect: avoid-ref-read-inside-build */
}
Object consumer() => Consumer(builder: (context, ref, child) => ref.read(countProvider)); /* expect: avoid-ref-read-inside-build */
class HookScreen extends hooks.HookConsumerWidget {
  Object build(Object context, hooks.WidgetRef ref) => ref.read(countProvider); /* expect: avoid-ref-read-inside-build */
}
