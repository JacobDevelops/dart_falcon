class A {
  int _a = 0;
  int b = 0;
  A? next;

  int joined() {
    return ++_a;
  }

  // `this.b` is not an identifier chain, matching the analyzer.
  int viaThis(int value) {
    this.b = value;
    return this.b;
  }

  // Mixing `b` and `this.b` is not provably the same element.
  int mixed(int value) {
    b = value;
    return this.b;
  }

  int? nullAsserted(A other) {
    other.next!.b = 4;
    return other.next!.b;
  }
}

// A preceding assignment to the same variable keeps the pair apart.
int runOfAssignments(int x) {
  x = 1;
  x += 2;
  return x;
}

int differentVariable(int x, int y) {
  x = 2;
  return y;
}

int notLast(int x) {
  x = 1;
  print(x);
  return x;
}

int indexed(List<int> list) {
  list[0] = 1;
  return list[0];
}

void noValue(int x) {
  x = 1;
  return;
}

// Switch-case statement lists are not blocks.
int inSwitch(int kind, int x) {
  switch (kind) {
    case 1:
      x = 3;
      return x;
    default:
      return 0;
  }
}

int callStatement(List<int> list) {
  list.add(1);
  return list.length;
}

class StateBase {
  int get state => 0;
  set state(int value) {}
}

class ReadOverride extends StateBase {
  @override
  int get state => super.state;
}

// The getter and setter canonicalize to variables on different declarations.
int splitAccessors(ReadOverride notifier) {
  notifier.state = 1;
  return notifier.state;
}

int splitAccessorChain(List<ReadOverride> notifiers) {
  notifiers.first.state = 1;
  return notifiers.first.state;
}

int differentReceivers(StateBase first, StateBase second) {
  first.state = 1;
  return second.state;
}

int differentAccessSyntax(StateBase notifier) {
  notifier.state = 1;
  return (notifier).state;
}
