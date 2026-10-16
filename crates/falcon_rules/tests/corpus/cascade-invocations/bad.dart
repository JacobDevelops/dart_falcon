class Point {
  int x = 0;
  int y = 0;
  void reset() {}
  void move(int dx) {}
}

class Holder {
  final List<int> items = [];

  void fill() {
    items.add(1);
    items.add(2); /* expect: cascade-invocations */
  }
}

void declared() {
  final list = <int>[];
  list.add(1); /* expect: cascade-invocations */
  list.add(2);
}

void parameter(Point p) {
  p.reset();
  p.move(1); /* expect: cascade-invocations */
}

void setters(Point p) {
  p.x = 1;
  p.y = 2; /* expect: cascade-invocations */
}

void assigned(Point p) {
  p = Point();
  p.reset(); /* expect: cascade-invocations */
}

// Source-defined contract keeps cascade resolution independent of SDK indexes.
class Buffer {
  void write(Object value) {}
}

void cascades(Buffer buffer) {
  buffer..write('a')..write('b');
  buffer.write('c'); /* expect: cascade-invocations */
}

void inSwitch(int value, List<int> list) {
  switch (value) {
    case 1:
      list.clear();
      list.add(value); /* expect: cascade-invocations */
  }
}

void broken(List<int> list, List<int> other) {
  list.add(1);
  list.add(2); /* expect: cascade-invocations */
  other.add(3);
  list.add(4);
}

// Members of another class than the receiver field's are no dependency.
class Out {
  void write(Object value) {}
}

class Writer {
  final Out buffer = Out();
}

class PrettyWriter extends Writer {
  final String indent = '  ';

  void write() {
    buffer.write('a');
    buffer.write(indent); /* expect: cascade-invocations */
  }
}

// A field receiver may take `this` or other objects' members as arguments.
class Field {
  final Point point = Point();

  void run(Point other) {
    point.reset();
    point.move(other.x); /* expect: cascade-invocations */
    print(this);
  }
}

// Assignment eligibility uses the receiver variable, not the setter element.
void dynamicSetters(dynamic receiver) {
  receiver.x = 1;
  receiver.y = 2; /* expect: cascade-invocations */
}

void dynamicObjectMethods(dynamic receiver) {
  receiver.toString();
  receiver.toString(); /* expect: cascade-invocations */
}

class ScopedReceiver {
  void initialize(void Function(ScopedReceiver) callback) {}
  void reset() {}
}

// The callback parameter is not a dependency on the outer receiver.
void shadowedCallback() {
  final receiver = ScopedReceiver();
  receiver.initialize((receiver) { /* expect: cascade-invocations */
    receiver.reset();
  });
  receiver.reset();
}
