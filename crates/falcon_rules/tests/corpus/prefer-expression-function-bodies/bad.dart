int twiceOf(int x) { /* expect: prefer-expression-function-bodies */
  return x * 2;
}

class Box {
  int _value = 0;
  int get value { /* expect: prefer-expression-function-bodies */
    return _value;
  }

  int twice() { /* expect: prefer-expression-function-bodies */
    return _value * 2;
  }

  Box operator +(Box other) { /* expect: prefer-expression-function-bodies */
    return Box();
  }

  Future<int> load() async { /* expect: prefer-expression-function-bodies */
    return _value;
  }
}

void main() {
  int local(int x) { /* expect: prefer-expression-function-bodies */
    return x + 1;
  }

  final f = (int x) { /* expect: prefer-expression-function-bodies */
    return x;
  };
  print(local(f(1)));
}
