void direct() {
  print('work');
  return; /* expect: avoid-unnecessary-return */
}
void branch(bool ready) {
  if (ready) {
    return; /* expect: avoid-unnecessary-return */
  }
}
void branches(bool ready) {
  if (ready) {
    return; /* expect: avoid-unnecessary-return */
  } else {
    print('work');
    return; /* expect: avoid-unnecessary-return */
  }
}
class Example {
  void method() {
    return; /* expect: avoid-unnecessary-return */
  }
  set value(int value) {
    print(value);
    return; /* expect: avoid-unnecessary-return */
  }
}
void closures() {
  final callback = () {
    return; /* expect: avoid-unnecessary-return */
  };
  callback();
}
void local() {
  void nested() {
    return; /* expect: avoid-unnecessary-return */
  }
  nested();
}
class Constructed {
  Constructed() {
    return; /* expect: avoid-unnecessary-return */
  }
}
