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
