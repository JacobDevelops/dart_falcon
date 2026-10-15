@deprecated
class OldDocBorder {}
@deprecated
typedef OldDocResolver<T> = T Function(int value);
class DocStyle {
  DocStyle.resolveWith();
}
@deprecated
typedef OldDocStyle = DocStyle;

// The preceding deprecated alias's parser span includes these comments.
/// A [OldDocBorder] created from [OldDocResolver<OldDocBorder>]. /* expect: deprecated-member-use-from-same-package */
/// Used by [OldDocStyle.resolveWith]. /* expect: deprecated-member-use-from-same-package */
class _DocBorder extends OldDocBorder { /* expect: deprecated-member-use-from-same-package */
  _DocBorder();
}

@deprecated
class OldDocServer {}
// An ordinary comment must not detach the documentation.
/// Returned by [OldDocServer]. /* expect: deprecated-member-use-from-same-package */
class _DocServer {}

/// A [OldDocResolver] and [OldDocStyle] reference. /* expect: deprecated-member-use-from-same-package */ /* expect: deprecated-member-use-from-same-package */
void docReferences() {}
