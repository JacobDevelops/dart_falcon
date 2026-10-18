import 'package:riverpod/riverpod.dart' as rp;
class CounterNotifier extends rp.Notifier<int> {}
class AsyncCounterNotifier extends rp.AsyncNotifier<int> {}
class StreamCounterNotifier extends rp.StreamNotifier<int> {}
class AutoCounterNotifier extends rp.AutoDisposeNotifier<int> {}
class ChildNotifier extends CounterNotifier {}
class _PrivateNotifier extends rp.Notifier<int> {}
class Notifier<T> {}
class Unrelated extends Notifier<int> {}
class AsyncNotifier<T> {}
class UnrelatedAsync extends AsyncNotifier<int> {}
class Ordinary {}
class Missing extends UnknownNotifier<int> {}
