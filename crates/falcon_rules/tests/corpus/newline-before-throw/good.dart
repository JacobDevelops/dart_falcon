void first() {
  throw StateError('first');
}

void separated() {
  print('before');

  throw StateError('after blank line');
}

void nested(bool condition) {
  if (condition) {
    throw StateError('first in nested block');
  }
  final callback = () {
    throw StateError('first in closure');
  };
  callback();
}

void guarded(bool condition) {
  if (condition) throw StateError('unbraced');
}

void cases(int value) {
  switch (value) {
    default:
      throw StateError('first');
  }
}

void followingCaseStatement(int value) {
  switch (value) {
    default:
      print(value);
      throw StateError('case');
  }
}

void commented() {
  print('before');

  // Explain the failure.
  throw StateError('failure');
}
