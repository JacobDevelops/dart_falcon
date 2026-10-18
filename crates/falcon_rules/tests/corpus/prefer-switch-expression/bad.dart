String describe(int value) {
  switch (value) { /* expect: prefer-switch-expression */
    case 1:
      return 'one';
    default:
      return 'other';
  }
}

int absolute(int value) {
  switch (value) { /* expect: prefer-switch-expression */
    case < 0:
      return -value;
    case _:
      return value;
  }
}

int guarded(Object value) {
  switch (value) { /* expect: prefer-switch-expression */
    case int number when number > 0:
      return number;
    default:
      return 0;
  }
}

int select(int value) {
  var result = 0;
  switch (value) { /* expect: prefer-switch-expression */
    case 1:
      result = 10;
      break;
    default:
      result = 20;
      break;
  }
  return result;
}

String choose(bool value) {
  var result = '';
  switch (value) { /* expect: prefer-switch-expression */
    case true:
      result = 'yes';
      break;
    case _:
      result = 'no';
      break;
  }
  return result;
}

Future<int> asynchronous(int value) async {
  switch (value) { /* expect: prefer-switch-expression */
    case 1:
      return await Future.value(1);
    default:
      return await Future.value(0);
  }
}

int implicitBreak(int value) {
  var result = 0;
  switch (value) { /* expect: prefer-switch-expression */
    case 1:
      result = 1;
    default:
      result = 2;
  }
  return result;
}

int independentBinding(Object value) {
  var result = 0;
  switch (value) { /* expect: prefer-switch-expression */
    case int number:
      result = number;
    default:
      result = -1;
      break;
  }
  return result;
}

int returningWithThrow(int value) {
  switch (value) { /* expect: prefer-switch-expression */
    case 1:
      return 1;
    default:
      throw StateError('Unsupported value');
  }
}

int assigningWithThrow(int value) {
  var result = 0;
  switch (value) { /* expect: prefer-switch-expression */
    case 0:
      throw StateError('Zero');
    case 1:
      result = 1;
      break;
    default:
      result = 2;
  }
  return result;
}
