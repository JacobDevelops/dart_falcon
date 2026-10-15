class Holder {
  final dynamic _value;
  Holder(this._value);

  void read() {
    if (this._value is String) {
      print(this._value.length);
    }
  }

  void readOther(Holder other) {
    if (other._value is String) {
      print(other._value.length);
    }
  }
}
