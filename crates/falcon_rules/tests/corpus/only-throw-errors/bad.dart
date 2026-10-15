class NotAnException {}

class Wrapper<T> {
  T value;
  Wrapper(this.value);
}

void throwsString() {
  throw 'Something went wrong'; /* expect: only-throw-errors */
}

void throwsInt() {
  throw 42; /* expect: only-throw-errors */
}

void throwsList() {
  throw ['a', 'b']; /* expect: only-throw-errors */
}

void throwsInterpolation(int code) {
  throw 'code $code'; /* expect: only-throw-errors */
}

void throwsCustom() {
  throw NotAnException(); /* expect: only-throw-errors */
}

int throwsInExpression(int? value) => value ?? (throw 'missing'); /* expect: only-throw-errors */

void throwsLocal() {
  final error = NotAnException();
  throw error; /* expect: only-throw-errors */
}

void throwsObject(Object error) {
  throw error; /* expect: only-throw-errors */
}

void throwsFunction(void Function() callback) {
  throw callback; /* expect: only-throw-errors */
}

void throwsRecord((int, String) value) {
  throw value; /* expect: only-throw-errors */
}

extension type ObjectValue(Object value) implements Object {}

void throwsObjectValue() {
  throw ObjectValue(Object()); /* expect: only-throw-errors */
}

// Declare the getter locally: this corpus runs without SDK member signatures.
// The inherited List.first cases live in the external-source inference tests.
class CollectedErrors {
  final _values = <Object>[];
  Object get first => _values.first;
  void add(Object error) => _values.add(error);
}

void throwsFirst(CollectedErrors errors) {
  throw errors.first; /* expect: only-throw-errors */
}

void throwsCollectedErrors() {
  final errors = CollectedErrors();
  try {
    throw Error();
  } catch (error) {
    errors.add(error);
  }
  throw errors.first; /* expect: only-throw-errors */
}
