class Provider<T> { Provider(T Function(Object) create); }
final count = Provider<int>((ref) => 0);
final ordinary = 0;
