/// Returns [valeu] doubled. /* expect: comment-references */
int twice(int value) => value * 2;

/// Wraps a [Strng] for display. /* expect: comment-references */
class Label {
  Label(this.text);

  /// The text; never [null]. /* expect: comment-references */
  final String text;

  /// Renders [text] with [prefx]. /* expect: comment-references */
  String render(String prefix) => '$prefix$text';

  /// See [Label.missing] for details. /* expect: comment-references */
  void describe() {}
}

/// Use [Unknown] instead, or [this]. /* expect: comment-references */ /* expect: comment-references */
void legacy() {}

/// Shades of [Colour]. /* expect: comment-references */
enum Shade {
  /// Brighter than [dakr]. /* expect: comment-references */
  light,
  dark,
}

/// A typedef's scope is closed: [day] and [T] resolve, [missingName] does not. /* expect: comment-references */
typedef DayPredicate<T> = bool Function(DateTime day, T value);
