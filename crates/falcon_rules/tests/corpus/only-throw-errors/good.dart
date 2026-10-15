class MyException implements Exception {}

class MyError extends Error {}

void throwsException() {
  throw Exception('Something went wrong');
}

void throwsFormatException() {
  throw FormatException('bad input');
}

void throwsStateError() {
  throw StateError('bad state');
}

void throwsCustomException() {
  throw MyException();
}

void throwsCustomError() {
  throw MyError();
}

void throwsDynamic(dynamic value) {
  throw value;
}

void throwsBounded<T extends Exception>(T error) {
  throw error;
}

Never fail() => throw UnsupportedError('nope');

int throwsInExpression(int? value) => value ?? (throw ArgumentError('missing'));

// Interface checks use the representation, even when only Object is implemented.
extension type ErrorValue(Error value) implements Object {}
extension type ExceptionValue(Exception value) implements Object {}
extension type NestedErrorValue(ErrorValue value) implements Object {}
extension type GenericErrorValue<T extends Error>(T value) implements Object {}

void throwsErrorValue() {
  throw ErrorValue(Error());
}

void throwsExceptionValue() {
  throw ExceptionValue(Exception());
}

void throwsNestedErrorValue() {
  throw NestedErrorValue(ErrorValue(Error()));
}

void throwsGenericErrorValue() {
  throw GenericErrorValue<StateError>(StateError('bad state'));
}

void throwsBoundedErrorValue<T extends ErrorValue>(T value) {
  throw value;
}
