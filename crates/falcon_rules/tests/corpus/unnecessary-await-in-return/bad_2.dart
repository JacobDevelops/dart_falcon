// An asserted `T?` infers `Future<T>`, and a type parameter's future is a
// future of its bound.
Future<S> wrap<S>(S value) async => value;
Future<T> asserted<T>(T? value) async => await wrap(value!); /* expect: unnecessary-await-in-return */
Future<T?> nullableResult<T>(Future<T> value) async => await value; /* expect: unnecessary-await-in-return */
Future<num> bounded<T extends num>(Future<T> value) async {
  return await value; /* expect: unnecessary-await-in-return */
}
