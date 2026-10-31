class AsyncValue<T> {
  bool get isLoading => false;
  bool get hasError => false;
  bool get hasValue => false;
  T get value => throw StateError('no value');
  Object get error => throw StateError('no error');
  R when<R>({required R Function() loading, required R Function(Object error, StackTrace stack) error, required R Function(T value) data}) => loading();
}
class AsyncLoading<T> extends AsyncValue<T> {}
class AsyncError<T> extends AsyncValue<T> {}
class AsyncData<T> extends AsyncValue<T> {}
