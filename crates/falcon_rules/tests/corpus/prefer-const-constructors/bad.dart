class Point {
  const Point(this.x, this.y);
  const Point.origin()
      : x = 0,
        y = 0;

  final int x;
  final int y;
}

class Label {
  const Label(this.text, {this.point = const Point.origin()});

  final String text;
  final Point point;
}

class Box<T> {
  const Box(this.value);

  final T value;
}

const int limit = 10;

void use(Object value) {}

void constructors() {
  final a = Point(1, 2); /* expect: prefer-const-constructors */
  final b = Point.origin(); /* expect: prefer-const-constructors */
  final c = new Point(3, 4); /* expect: prefer-const-constructors */
  final d = new Point.origin(); /* expect: prefer-const-constructors */
  final e = Box<int>(limit); /* expect: prefer-const-constructors */
  final f = Label('title', point: const Point(0, 1)); /* expect: prefer-const-constructors */
  use(Point(limit, -1)); /* expect: prefer-const-constructors */
  final List<Point> points = [Point(5, 6)]; /* expect: prefer-const-constructors */
  final Point g = .origin(); /* expect: prefer-const-constructors */
  print([a, b, c, d, e, f, points, g]);
}

enum Shade { dark, light }

class Swatch {
  const Swatch(this.value, {this.shade});
  const Swatch.named(this.value) : shade = null;
  const factory Swatch.redirect(Object? value) = Swatch;
  static const int base = 1;
  final Object? value;
  final Shade? shade;
}

class Generic<T> {
  const Generic(this.value);
  const Generic.named(this.value);
  final T value;
}

int top() => 0;

void evaluatedArguments() {
  const local = 2;
  print(Swatch(Shade.dark)); /* expect: prefer-const-constructors */
  print(Swatch(Swatch.base)); /* expect: prefer-const-constructors */
  print(Swatch(local)); /* expect: prefer-const-constructors */
  print(Generic<int>.named(1)); /* expect: prefer-const-constructors */
  print(Swatch.redirect(1)); /* expect: prefer-const-constructors */
  print(Swatch('a' 'b')); /* expect: prefer-const-constructors */
  print(Swatch(#symbol)); /* expect: prefer-const-constructors */
  print(Swatch(int)); /* expect: prefer-const-constructors */
  print(Swatch(top)); /* expect: prefer-const-constructors */
  print(Swatch(Swatch.named)); /* expect: prefer-const-constructors */
  print(Swatch(Shade.values)); /* expect: prefer-const-constructors */
  print(Swatch(identical(1, 2))); /* expect: prefer-const-constructors */
  print(Swatch([if (true) 1])); /* expect: prefer-const-constructors */
  print(Swatch({Swatch(1): 1})); /* expect: prefer-const-constructors */ /* expect: prefer-const-constructors */
  final generic = Generic(1); /* expect: prefer-const-constructors */
  print(generic);
}

// The context type `Box<List<T>>` has no type parameter as a type argument
// of `Box` itself, so making it `const` changes nothing.
Box<List<T>> emptyBox<T>() => Box([]); /* expect: prefer-const-constructors */
