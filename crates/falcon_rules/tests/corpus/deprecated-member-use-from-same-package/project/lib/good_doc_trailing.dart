@deprecated
class OldGoodDocBorder {}
@deprecated
typedef OldGoodDocResolver<T> = T Function(int value);

// Generic bracket contents are not analyzer documentation references.
/// A [OldGoodDocResolver<OldGoodDocBorder>].
class _GoodDocBorder {}

/// Uses [OldGoodDocBorder] within a deprecated declaration.
@deprecated
class DeprecatedDocOwner {
  /// Uses [OldGoodDocBorder] within a deprecated member's owner.
  void member() {}
}
