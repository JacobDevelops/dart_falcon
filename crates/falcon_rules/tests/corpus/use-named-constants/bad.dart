class Point {
  const Point(this.x, this.y);
  final int x;
  final int y;

  static const origin = Point(0, 0);
  static const unitX = Point(1, 0);
}

class Flag {
  const Flag();
  const Flag.named();

  static const instance = Flag();
}

const a = Point(0, 0); /* expect: use-named-constants */

void main() {
  const b = Point(1, 0); /* expect: use-named-constants */
  final c = const Point(0, 0); /* expect: use-named-constants */
  print(const Flag()); /* expect: use-named-constants */
  const list = [Point(0, 0)]; /* expect: use-named-constants */
  print([a, b, c, list, const Flag.named()]); /* expect: use-named-constants */
}

class Holder {
  const Holder([this.p = const Point(1, 0)]); /* expect: use-named-constants */
  final Point p;
}

class Initialized {
  const Initialized(this.x);
  const Initialized.named() : x = 0;
  final int x;
  static const zero = Initialized(0);
}

const sameFields = Initialized.named(); /* expect: use-named-constants */

abstract class NamedSequence<T> {
  const factory NamedSequence.single(T value) = SingleSequence<T>;
  static const trueSequence = SingleSequence<bool>(true);
  static const emptyString = SingleSequence<String>('');
}

class SingleSequence<T> implements NamedSequence<T> {
  const SingleSequence(this.value);
  final T value;
}

const redirectedBool = NamedSequence.single(true); /* expect: use-named-constants */
const redirectedString = NamedSequence.single(''); /* expect: use-named-constants */

const Initialized contextualShorthand = .named(); /* expect: use-named-constants */
final Initialized explicitShorthand = const .named(); /* expect: use-named-constants */
