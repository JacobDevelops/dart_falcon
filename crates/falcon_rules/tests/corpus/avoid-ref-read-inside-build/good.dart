import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Screen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    final callback = () => ref.read(countProvider);
    void local() { ref.read(countProvider); }
    return ref.watch(countProvider);
  }
  int action(WidgetRef ref) => ref.read(countProvider);
}
int outside(WidgetRef ref) => ref.read(countProvider);
final plain = Provider<int>((ref) => ref.read(countProvider));
class Unrelated {
  int read(Object value) => 0;
  Object build(Object context, Unrelated ref) => ref.read(countProvider);
}
Object consumer() => Consumer(builder: (context, ref, child) => () => ref.read(countProvider));
class DeferredMenu extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) =>
      PopupMenuButton(itemBuilder: (context) => ref.read(countProvider));
}
