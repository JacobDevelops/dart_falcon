class P {
  int x = 0;
}

extension Generic<T extends P> on T {
  void setX(int value) => this.x = value; /* expect: use-setters-to-change-properties */
  void implicit(int value) => x = value; /* expect: use-setters-to-change-properties */
}

extension Nullable on P? {
  void setX(int value) => this!.x = value; /* expect: use-setters-to-change-properties */
  void nullAware(int value) => this?.x = value; /* expect: use-setters-to-change-properties */
  set y(int value) {}
  void setY(int value) => this.y = value; /* expect: use-setters-to-change-properties */
}

class NullableHolder {
  final P? target = null;
  void change(int value) => target?.x = value; /* expect: use-setters-to-change-properties */
  void asserted(int value) => target!.x = value; /* expect: use-setters-to-change-properties */
  void groupedAssertion(int value) => (target)!.x = value; /* expect: use-setters-to-change-properties */
}
