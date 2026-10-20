abstract class Notifier<T> {}
abstract class AsyncNotifier<T> {}
abstract class StreamNotifier<T> {}
abstract class AutoDisposeNotifier<T> extends Notifier<T> {}
abstract class AutoDisposeAsyncNotifier<T> extends AsyncNotifier<T> {}
abstract class NotifierBase<T> {}
class ProviderBase<T> {}
class Ref {}
class Provider<T> extends ProviderBase<T> { Provider(T Function(Ref) create); }
class FutureProvider<T> extends ProviderBase<T> { FutureProvider(T Function(Ref) create); }
class StreamProvider<T> extends ProviderBase<T> { StreamProvider(T Function(Ref) create); }
class StateProvider<T> extends ProviderBase<T> { StateProvider(T Function(Ref) create); }
class NotifierProvider<N, T> extends ProviderBase<T> { NotifierProvider(N Function() create); }
class ProviderFamily<T, A> { ProviderFamily(T Function(Ref, A) create); }
class AutoDisposeProvider<T> extends ProviderBase<T> { AutoDisposeProvider(T Function(Ref) create); }
