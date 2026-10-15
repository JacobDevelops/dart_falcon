class Equality<T> {
  const Equality();
  bool equals(List<T> a, List<T> b) => true;
}

// A receiver has no downward context; absence must not become Unknown context.
bool compare(List<int> a, List<int> b) =>
    Equality().equals(a, b); /* expect: prefer-const-constructors */

class Box<T> {
  const Box(this.value);
  final T value;
}

// Real contextual unknown type parameters must still prevent a const rewrite.
Box<T?> contextual<T>() => Box(null);

void argument<T>(void Function(Box<T?>) sink) {
  sink(Box(null));
}
