abstract class Ref { T read<T>(Object provider); }
abstract class Notifier<T> { T build(); T get state; set state(T value); Ref get ref; }
abstract class AsyncNotifier<T> extends Notifier<T> {}
abstract class StreamNotifier<T> extends Notifier<T> {}
abstract class StateNotifier<T> { T get state; }
class Provider<T> {}
class ProviderFamily<T, A> { Provider<T> call(A argument) => Provider<T>(); }
class FutureProviderFamily<T, A> extends ProviderFamily<T, A> {}
class AutoDisposeProviderFamily<T, A> extends ProviderFamily<T, A> {}
