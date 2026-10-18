String expression(int value) => switch (value) {
  1 => 'one',
  _ => 'other',
};

// Even if a type could prove exhaustiveness, this rule requires a fallback.
int noFallback(bool value) {
  switch (value) {
    case true:
      return 1;
    case false:
      return 0;
  }
}

int grouped(int value) {
  switch (value) {
    case 1:
    case 2:
      return 1;
    default:
      return 0;
  }
}

void empty(int value) {
  switch (value) {
    case 1:
      break;
    default:
  }
}

int extraWork(int value) {
  switch (value) {
    case 1:
      print(value);
      return 1;
    default:
      return 0;
  }
}

void noValues(int value) {
  switch (value) {
    case 1:
      return;
    default:
      return;
  }
}

int differentTargets(int value) {
  var first = 0;
  var second = 0;
  switch (value) {
    case 1:
      first = 1;
      break;
    default:
      second = 2;
      break;
  }
  return first + second;
}

int compound(int value) {
  var result = 0;
  switch (value) {
    case 1:
      result += 1;
      break;
    default:
      result += 2;
      break;
  }
  return result;
}

class Box {
  int value = 0;
}

void properties(int value, Box box) {
  switch (value) {
    case 1:
      box.value = 1;
      break;
    default:
      box.value = 2;
      break;
  }
}

void indexed(int value, List<int> values) {
  switch (value) {
    case 1:
      values[0] = 1;
      break;
    default:
      values[0] = 2;
      break;
  }
}

int mixed(int value) {
  var result = 0;
  switch (value) {
    case 1:
      return 1;
    default:
      result = 2;
      break;
  }
  return result;
}

int blocks(int value) {
  switch (value) {
    case 1:
      { return 1; }
    default:
      { return 0; }
  }
}

int labeledBreak(int value) {
  var result = 0;
  outer: while (true) {
    switch (value) {
      case 1:
        result = 1;
        break outer;
      default:
        result = 2;
        break outer;
    }
  }
  return result;
}

int labeledCase(int value) {
  switch (value) {
    target: case 1:
      return 1;
    default:
      return 0;
  }
}

int guardedWildcard(int value) {
  switch (value) {
    case _ when value > 0:
      return 1;
    case 0:
      return 0;
  }
  return -1;
}

int typedWildcard(Object value) {
  switch (value) {
    case int _:
      return 1;
    case String _:
      return 2;
  }
  return 0;
}

// Same spelling does not imply the same variable in different patterns.
int patternBinding(Object value) {
  var result = 0;
  switch (value) {
    case int result:
      result = 1;
      break;
    default:
      result = 2;
      break;
  }
  return result;
}

Never onlyThrows(int value) {
  switch (value) {
    case 1:
      throw StateError('One');
    default:
      throw StateError('Other');
  }
}

int throwWithExtraStatement(int value) {
  switch (value) {
    case 1:
      print(value);
      throw StateError('One');
    default:
      return 0;
  }
}

int implicitAssignmentWithExtraStatement(int value) {
  var result = 0;
  switch (value) {
    case 1:
      result = 1;
      print(result);
    default:
      result = 2;
  }
  return result;
}
