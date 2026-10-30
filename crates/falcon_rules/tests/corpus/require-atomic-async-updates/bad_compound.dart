// A compound assignment reads its own target without needing to compare it
// with a separate expression, even when its receiver cannot be resolved.
import 'package:unavailable/state.dart';

class Consumer extends ExternalState {
  Future<void> update(Future<int> amount) async {
    first += await amount; /* expect: require-atomic-async-updates */
    first[0] += await amount; /* expect: require-atomic-async-updates */
    first.count += await amount; /* expect: require-atomic-async-updates */
  }
}

class Counter {
  int value = 0;
}

Future<void> complexTargets(Future<int> amount) async {
  final values = <int>[0, 1];
  var index = 0;
  values[index + 1] += await amount; /* expect: require-atomic-async-updates */
  Counter().value += await amount; /* expect: require-atomic-async-updates */
  final key = Counter();
  values[key.value] += await amount; /* expect: require-atomic-async-updates */
}
