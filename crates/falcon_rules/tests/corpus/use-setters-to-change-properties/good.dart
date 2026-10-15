class Rectangle {
  num _width = 0;
  num _height = 0;

  set width(num value) => _width = value;
  num get width => _width;

  // Not a plain assignment of the parameter.
  void doubleHeight(num value) => _height = value * 2;

  // More than one statement.
  void setBoth(num value) {
    _width = value;
    _height = value;
  }

  // Compound assignment.
  void grow(num value) => _height += value;

  // Non-void return type.
  num setHeight(num value) => _height = value;

  // Two parameters.
  void resize(num w, num h) => _width = w;

  num get height => _height;
}

// Overriding an inherited method.
abstract class Settable {
  void setValue(int value);
}

class Impl implements Settable {
  int value = 0;

  @override
  void setValue(int value) => this.value = value;
}

// Assigning a top-level variable is not a property change.
int globalCount = 0;

class Counter {
  void setGlobal(int value) => globalCount = value;
}


// Implementing an SDK interface member.
class Recorder implements Sink<int> {
  int last = 0;
  void add(int data) {
    last = data;
  }

  void close() {}
}

class ExtensionTarget {
  int value = 0;
}

extension ChangeValue on ExtensionTarget {
  void updateLocal(int newValue) {
    var value = 0;
    value = newValue;
  }

  void updateGlobal(int newValue) => globalCount = newValue;
}
