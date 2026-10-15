class SetterTarget {
  int value = 0;
  set writeOnly(int value) {}
}

extension Change on SetterTarget {
  @override
  // ignore: override_on_non_overriding_member
  void change(int newValue) => value = newValue; /* expect: use-setters-to-change-properties */
  void explicitSetter(int newValue) => this.writeOnly = newValue; /* expect: use-setters-to-change-properties */
  void implicitSetter(int newValue) => writeOnly = newValue; /* expect: use-setters-to-change-properties */
}

class Holder {
  final SetterTarget target = SetterTarget();
  void change(int value) => target.writeOnly = value; /* expect: use-setters-to-change-properties */
}
