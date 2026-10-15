/// Returns [value] doubled, see [Label] and [print].
int twice(int value) => value * 2;

/// A label holding [text]; compare with [Label.render] and [Object].
///
/// Code is not checked: `[notAReference]` and
///
/// ```dart
/// final list = [missing];
/// ```
///
/// Links are not references: [the docs](https://dart.dev) and [guide][].
///
/// [guide]: https://dart.dev/guides
class Label<T> {
  Label(this.text);

  /// The text, typed [T]; equal [hashCode] values for equal [text].
  final String text;

  /// Renders [text] after [prefix] using [twice] and [Shade.dark].
  String render(String prefix) => '$prefix$text';
}

/// Mentions [Shade.values], [index], [light], and [toString].
enum Shade {
  /// Unlike [dark].
  light,
  dark,
}

/// A [FancyLabel] inherits [render] and [text] from [Label].
class FancyLabel extends Label<int> {
  FancyLabel(super.text);
}

/// [List] of [int] values in a [Map], with [Duration] and [identical].
const values = <int>[];

/// Built by [Point.new], [Implicit.new], [Factory.new], [Tone.new],
/// [Wrapper.new], [AbstractBase.new], and [Named.new].
class Point {
  Point(this.x);

  final int x;
}

class Implicit {}

abstract class AbstractBase {}

class Factory {
  Factory._();

  factory Factory.new() => Factory._();
}

enum Tone { low, high }

extension type Wrapper(int value) {}

extension type Named._(int value) {
  Named(int value) : this._(value);
}
