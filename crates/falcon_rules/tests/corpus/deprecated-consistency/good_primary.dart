// @dart = 3.13

class Plain();

@deprecated
class Marked() {
  @deprecated
  this {}
}

class Both(@deprecated this.value) {
  @deprecated
  int value;
}

class Private(this._value) {
  @deprecated
  int _value;
}

class Declaring(final int value) {
  Declaring.other(int value) : this(value);
}

