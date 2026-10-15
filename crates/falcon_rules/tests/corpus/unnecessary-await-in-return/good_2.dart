// A nullable type parameter's future is no future of the non-nullable one.
Future<T> nullable<T>(Future<T?> value) async => (await value)!;
Future<int> unrelated<T extends num>(Future<T> value) async => await value as int;
