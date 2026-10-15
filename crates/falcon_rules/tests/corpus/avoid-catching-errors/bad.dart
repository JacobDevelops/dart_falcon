void work() {}

class MyError extends Error {}

class MyStateError extends StateError {
  MyStateError(super.message);
}

void catchesError() {
  try {
    work();
  } on Error catch (e) { /* expect: avoid-catching-errors */
    print(e);
  }
}

void catchesStateError() {
  try {
    work();
  } on StateError catch (e) { /* expect: avoid-catching-errors */
    print(e);
  }
}

void catchesArgumentError() {
  try {
    work();
  } on ArgumentError { /* expect: avoid-catching-errors */
    print('bad argument');
  }
}

void catchesCustomError() {
  try {
    work();
  } on MyError catch (e) { /* expect: avoid-catching-errors */
    print(e);
  }
}

void catchesNestedCustomError() {
  try {
    work();
  } on MyStateError catch (e) { /* expect: avoid-catching-errors */
    print(e);
  }
}

void catchesTypeError() {
  try {
    work();
  } on TypeError catch (e) { /* expect: avoid-catching-errors */
    print(e);
  }
}

extension type WrappedStateError(StateError error) implements StateError {}
extension type WrappedError(Error error) implements Object {}

void catchesWrappedErrors() {
  try {} on WrappedStateError {} /* expect: avoid-catching-errors */
  try {} on WrappedError catch (_) {} /* expect: avoid-catching-errors */
}
