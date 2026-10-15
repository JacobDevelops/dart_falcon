T force<T>(T? value) {
  T result = value!; /* expect: null-check-on-nullable-type-parameter */
  T assigned;
  assigned = value!;
  return result;
}

T returnForce<T>(T? value) {
  return value!; /* expect: null-check-on-nullable-type-parameter */
}

void pattern<T>(T? value) {
  if (value case var item!) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
  }
}

class Holder<T> {
  T field;
  T force(T? value) => value!;

  void assign(Holder<T> self, T? value) {
    self.field = value!; /* expect: null-check-on-nullable-type-parameter */
  }
}

T closureContexts<T>(T? value) {
  final T Function(T?) callback = (T? inner) => inner!;
  final T Function<T>(T?) generic = <T>(T? inner) => inner!;
  callback(value);
  generic<T>(value);
  return value!; /* expect: null-check-on-nullable-type-parameter */
}

class PatternBox<T> {
  PatternBox(this.value);
  final T? value;
}

void nestedPatterns<T>(
  (T?,) record,
  List<T?> list,
  Map<String, T?> map,
  PatternBox<T> box,
) {
  if (record case (var item!,)) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
  }
  if (list case [var item!]) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
  }
  if (map case {'x': var item!}) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
  }
  if (box case PatternBox(value: var item!)) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
  }
}

void acceptGeneric<T>(T Function() callback) {}

void explicitCallType<T>(T? value) {
  acceptGeneric<T>(() => value!);
}

T? keepNullable<T>(T? value) => value!; /* expect: null-check-on-nullable-type-parameter */
void nullableAssignment<T>(T? value) {
  T? result;
  result = value!; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}

class Entry<T> {
  T? key;
}
void compare<T>(int Function(T, T) comparator, T key, Entry<T> entry) {
  comparator(key, entry.key!); /* expect: null-check-on-nullable-type-parameter */
}

T assignment<T>(T? value) {
  T result;
  result = value!; /* expect: null-check-on-nullable-type-parameter */
  return result;
}
