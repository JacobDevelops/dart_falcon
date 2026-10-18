import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Counter extends Notifier<int> { void increment() {} void reset() {} int get value => 0; }
class AsyncCounter extends AsyncNotifier<int> { void refresh() {} }
final counterProvider = Provider<Counter>((ref) => Counter());
final asyncProvider = Provider<AsyncCounter>((ref) => AsyncCounter());
class Service { void increment() {} }
class Screen extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) {
    final counter = ref.watch(counterProvider);
    final callback = () => counter.increment();
    void local() { counter.reset(); }
    final tearoff = counter.increment;
    final service = Service();
    service.increment();
    counter.toString();
    return callback;
  }
  void action(WidgetRef ref) { ref.read(counterProvider).increment(); }
}
void outside(Counter counter) { counter.increment(); }
class Unrelated {
  Object build(Object context, Counter counter) { counter.increment(); return 0; }
}
class DeferredMenu extends ConsumerWidget {
  Object build(Object context, WidgetRef ref) =>
      PopupMenuButton(itemBuilder: (context) {
        ref.read(counterProvider).increment();
        return 0;
      });
}
