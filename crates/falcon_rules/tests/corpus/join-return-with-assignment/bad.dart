class A {
  int _a = 0;
  int b = 0;
  late A next;

  int incrementA() {
    _a++; /* expect: join-return-with-assignment */
    return _a;
  }

  int plusEquals(int value) {
    _a += value; /* expect: join-return-with-assignment */
    return _a;
  }

  int assignB(int value) {
    b = value; /* expect: join-return-with-assignment */
    return b;
  }

  int? chained(A other) {
    other.b = 3; /* expect: join-return-with-assignment */
    return other.b;
  }

  int? propertyChain(A other) {
    print('x');
    other.next.b = 4; /* expect: join-return-with-assignment */
    return other.next.b;
  }
}

int prefix(int x) {
  ++x; /* expect: join-return-with-assignment */
  return x;
}

int afterOtherStatement(int x, int y) {
  y = 2;
  x = y + 1; /* expect: join-return-with-assignment */
  return x;
}

int nested(bool c, int x) {
  if (c) {
    x *= 2; /* expect: join-return-with-assignment */
    return x;
  }
  return 0;
}

int? nullable(int? x) {
  x ??= 1; /* expect: join-return-with-assignment */
  return x;
}

int Function() closure(int x) {
  return () {
    x--; /* expect: join-return-with-assignment */
    return x;
  };
}
