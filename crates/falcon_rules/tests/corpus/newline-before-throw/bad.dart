void fail(bool condition) {
  print('start');
  throw StateError('failure'); /* expect: newline-before-throw */
}

void nested(bool condition) {
  if (condition) {
    print('nested');
    throw StateError('nested'); /* expect: newline-before-throw */
  }
  final callback = () {
    print('closure');
    throw StateError('closure'); /* expect: newline-before-throw */
  };
  callback();
}

void cases(int value) {
  switch (value) {
    case 0: {
      print(value);
      throw StateError('case block'); /* expect: newline-before-throw */
    }
    default:
      throw StateError('first');
  }
}

void commented() {
  print('before');
  // No separating blank line.
  throw StateError('failure'); /* expect: newline-before-throw */
}
