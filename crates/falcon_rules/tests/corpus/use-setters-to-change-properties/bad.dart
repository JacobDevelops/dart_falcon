class Rectangle {
  num width = 0;
  num _height = 0;

  void setWidth(num width) { /* expect: use-setters-to-change-properties */
    this.width = width;
  }

  void changeHeight(num value) => _height = value; /* expect: use-setters-to-change-properties */

  void updateWidth(num value) { /* expect: use-setters-to-change-properties */
    width = value;
  }

  num get height => _height;
}

class Base {
  int count = 0;
}

// Inherited field assigned through `this`.
class Derived extends Base {
  void setCount(int value) => this.count = value; /* expect: use-setters-to-change-properties */
}

mixin Named {
  String name = '';

  void rename(String newName) => name = newName; /* expect: use-setters-to-change-properties */
}

// Assigning through an explicit setter.
class Box {
  int _size = 0;
  set size(int value) => _size = value;
  int get size => _size;

  void resize(int value) { /* expect: use-setters-to-change-properties */
    size = value;
  }
}

class Remote extends SomeUnknownBase {
  int port = 0;
  void setPort(int value) => port = value; /* expect: use-setters-to-change-properties */
}

class PropertyBase {
  int port = 0;
}
class InheritedProperty extends PropertyBase {
  void setPort(int value) => port = value; /* expect: use-setters-to-change-properties */
}

class Payload {
  int value = 0;
}
class WritesOther {
  final Payload payload = Payload();
  void update(int value) => payload.value = value; /* expect: use-setters-to-change-properties */
  void updateArgument(Payload other) => other.value = other.value;
}

// Resolve the implicit property target through a locally declared extension.
class ExtensionTarget {
  int value = 0;
}

extension ChangeValue on ExtensionTarget {
  void changeValue(int newValue) { /* expect: use-setters-to-change-properties */
    value = newValue;
  }

  void changeExplicitly(int newValue) => this.value = newValue; /* expect: use-setters-to-change-properties */
}
