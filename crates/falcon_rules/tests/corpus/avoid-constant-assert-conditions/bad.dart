const enabled = true;
void example() {
  assert(enabled); /* expect: avoid-constant-assert-conditions */
  assert(1 == 2, 'failure'); /* expect: avoid-constant-assert-conditions */
  assert(true); /* expect: avoid-constant-assert-conditions */
}
class Example {
  Example() : assert(enabled); /* expect: avoid-constant-assert-conditions */
  const Example.named() : assert(1 < 2); /* expect: avoid-constant-assert-conditions */
  Example.other() : assert(false); /* expect: avoid-constant-assert-conditions */
}

class Settings {
  static const enabled = true;
  const Settings() : assert(enabled); /* expect: avoid-constant-assert-conditions */
}

void constAllocationAssertions() {
  assert(const <int>[] == const <int>[]); /* expect: avoid-constant-assert-conditions */
}
