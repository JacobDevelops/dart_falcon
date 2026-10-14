class Holder {
  dynamic value;
  Holder(this.value);
  void set(v) {}
  set current(v) {}
}

void log(Object? message) {}

void untyped(message) {}

dynamic decode(String json) => json;

void callback(dynamic f()) {}

typedef Legacy(event);

typedef Callback = void Function(Object? event);

void main() {
  final f = (x) => x;
  final List<dynamic> list = [];
  dynamic local = 1;
  print([f, list, local]);
}
