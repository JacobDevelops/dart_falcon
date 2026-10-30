import 'state.dart' as state;
Future<void> update(state.Counter counter, state.Counter other, Future<int> amount) async {
  state.count = state.count + await amount; /* expect: require-atomic-async-updates */
  counter.value = counter.value + await amount; /* expect: require-atomic-async-updates */
  counter.value = other.value + await amount;
  state.count = await amount + state.count;
}
