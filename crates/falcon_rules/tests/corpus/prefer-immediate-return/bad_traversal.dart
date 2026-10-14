final callback = () {
  final value = compute(); /* expect: prefer-immediate-return */
  return value;
};

void finallyBody() {
  try {} finally {
    final value = compute(); /* expect: prefer-immediate-return */
    return value;
  }
}

class ReturningOperator {
  int operator +(int other) {
    final value = compute(); /* expect: prefer-immediate-return */
    return value;
  }
}
