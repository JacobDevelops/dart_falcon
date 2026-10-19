class Secret {
  int _value = 0;
  static int _shared = 0;
  void _reset() {}
  static Secret _default() => Secret();
}
class Other {
  void inspect(Secret secret) {
    final value = secret._value; /* expect: avoid-accessing-other-classes-private-members */
    secret._value = value; /* expect: avoid-accessing-other-classes-private-members */
    secret._reset(); /* expect: avoid-accessing-other-classes-private-members */
    final reset = secret._reset; /* expect: avoid-accessing-other-classes-private-members */
    Secret._shared++; /* expect: avoid-accessing-other-classes-private-members */
    secret.._value = 1; /* expect: avoid-accessing-other-classes-private-members */
    secret.._reset(); /* expect: avoid-accessing-other-classes-private-members */
  }
}
int outside(Secret secret) => secret._value; /* expect: avoid-accessing-other-classes-private-members */

class NullableAccess {
  void inspect(Secret? secret) {
    secret?._value; /* expect: avoid-accessing-other-classes-private-members */
    secret?._reset(); /* expect: avoid-accessing-other-classes-private-members */
  }
  void shorthand() {
    Secret secret = ._default(); /* expect: avoid-accessing-other-classes-private-members */
  }
}
class Child extends Secret {
  int inspectBase(Secret secret) => secret._value; /* expect: avoid-accessing-other-classes-private-members */
}
