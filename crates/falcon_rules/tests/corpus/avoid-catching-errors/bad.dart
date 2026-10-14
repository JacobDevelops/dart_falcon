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
