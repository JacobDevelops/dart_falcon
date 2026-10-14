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

void throwsNull() {
  throw null; /* expect: only-throw-errors */
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
