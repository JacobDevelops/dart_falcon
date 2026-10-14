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
