void main() {
  final used = Used();
  print(used + 1);
  print(1 + 2);
  print([1, 2][0]);
  print(used?.value());
}

class Used {
  Used operator +(int value) => this;
  int value() => 1;
  int operator [](int index) => index; /* expect: unreachable-from-main */
}

class Other { /* expect: unreachable-from-main */
  Other operator +(int value) => this;
  int value() => 2;
}

extension UnusedNullable on Used? { /* expect: unreachable-from-main */
  int value() => 3;
}
