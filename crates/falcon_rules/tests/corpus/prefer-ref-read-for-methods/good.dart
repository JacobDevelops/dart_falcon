import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;
final countProvider = Provider<int>((ref) => 0);
class Counter extends Notifier<int> { void increment() {} void reset() {} int get value => 0; }
class AsyncCounter extends AsyncNotifier<int> { void refresh() {} }
final counterProvider = Provider<Counter>((ref) => Counter());
final asyncProvider = Provider<AsyncCounter>((ref) => AsyncCounter());
void actions(WidgetRef ref) {
  ref.read(counterProvider).increment();
  final increment = ref.read(counterProvider).increment;
  final state = ref.watch(counterProvider).value;
  final notifier = ref.watch(counterProvider);
  notifier.increment();
  ref.watch(counterProvider).toString();
}
class Service { void invoke() {} }
final serviceProvider = Provider<Service>((ref) => Service());
void service(WidgetRef ref) { ref.watch(serviceProvider).invoke(); }
void unknown(dynamic ref) { ref.watch(counterProvider).increment(); }
