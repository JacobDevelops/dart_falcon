class Rectangle {
  final int width;
  final int height;
  Rectangle(this.width, this.height);
  Rectangle.square(int size)
      : width = size,
        height = size;
}

class ColoredRectangle extends Rectangle {
  final String color;
  ColoredRectangle(
    this.color,
    super.height, /* expect: matching-super-parameters */
    super.width, /* expect: matching-super-parameters */
  );
}

class Square extends Rectangle {
  Square(super.side) : super.square(); /* expect: matching-super-parameters */
}

class Base {
  Base(int a, [int? b]);
}

class Child extends Base {
  Child(super.x); /* expect: matching-super-parameters */
}

class OptionalChild extends Base {
  OptionalChild(super.a, [super.c]); /* expect: matching-super-parameters */
}

class PrivateBase {
  PrivateBase(int _channel);
}
class PublicChild extends PrivateBase {
  PublicChild(super.channel); /* expect: matching-super-parameters */
}

class PrimaryMismatch(super.height, super.width) extends Rectangle; /* expect: matching-super-parameters */ /* expect: matching-super-parameters */

class NamedPrimaryMismatch(super.side) extends Rectangle { /* expect: matching-super-parameters */
  this : super.square();
}
