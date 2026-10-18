import 'package:flutter_hooks/flutter_hooks.dart' as flutter_hooks;
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Screen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    final value = ref.watch(countProvider);
    return value;
  }
  int action(WidgetRef ref) => ref.read(countProvider);
}
class ScreenState extends ConsumerState<Object> {
  Object build(Object context) => ref.watch(countProvider);
}
final dependent = Provider<int>((ref) => ref.watch(countProvider));
Object consumer() => Consumer(builder: (context, ref, child) => ref.watch(countProvider));
class Unrelated { int watch(Object value) => 0; }
int unrelated(Unrelated ref) => ref.watch(countProvider);
int unknown(dynamic ref) => ref.watch(countProvider);
class BuilderScreen extends ConsumerWidget {
  static final dependent = Provider<int>((Ref ref) => ref.watch(countProvider));
  Object build(Object context, WidgetRef ref) =>
      Builder(builder: (context) => ref.watch(countProvider));
}
class SliverBuilderScreen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) =>
      SliverChildBuilderDelegate((context, index) => ref.watch(countProvider));
}
class OtherBuilders extends hooks.HookConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    SliverList.builder(itemBuilder: (context, index) => ref.watch(countProvider));
    SliverGrid.builder(itemBuilder: (context, index) => ref.watch(countProvider));
    ReorderableListView.builder(itemBuilder: (context, index) => ref.watch(countProvider));
    AnimatedList(itemBuilder: (context, index, animation) => ref.watch(countProvider));
    return flutter_hooks.HookBuilder(builder: (context) => ref.watch(countProvider));
  }
}
