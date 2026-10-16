// @dart=3.12
class Marker {
  const Marker();
}
const marker = Marker();

void documented(
  /// The parameter documentation is part of the parameter node. /* expect: prefer-final-parameters */
  @marker
  int value,
) {}

void finalized(
  /// No lint for an already final parameter.
  @marker
  final int value,
) {}

void modifiers({
  /// Documentation before required. /* expect: prefer-final-parameters */
  required int first,
  required /* expect: prefer-final-parameters */
  /// Documentation after required starts at the keyword, not this comment.
  int second,
  @marker /* expect: prefer-final-parameters */
  /// Documentation after metadata starts at the annotation.
  int third = 0,
}) {}

class DocumentedSetter {
  set first(
    /// Documentation before parameter metadata. /* expect: prefer-final-parameters */
    @marker covariant int value
  ) {}
  set second(
    /// Documentation before covariance. /* expect: prefer-final-parameters */
    covariant int value
  ) {}
}
