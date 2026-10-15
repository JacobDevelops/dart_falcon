extension type WrappedError(Error value) implements Object {}
extension type Wrapped<T extends Object>(T value) implements Object {}
extension type Number(int value) implements Object {}

void throwError(Error error) {
  throw WrappedError(error);
}

void throwGenericError(Error error) {
  throw Wrapped<Error>(error);
}

void throwNumber() {
  throw Number(1); /* expect: only-throw-errors */
}

void throwBoundError<T extends WrappedError>(T error) {
  throw error;
}
