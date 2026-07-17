// Public API surface exercising unused-code.

// Referenced from main.dart → live.
class PublicUsed {}

// Never referenced anywhere else → dead public class.
class PublicUnused {} /* expect: unused-code */

// Private: never a candidate, regardless of usage.
class _PrivateClass {}

// Annotated declarations are exempt (annotations often imply framework use).
@Deprecated('legacy')
class AnnotatedUnused {}

// Referenced from main.dart → live.
void usedFn() {}

// Never referenced elsewhere → dead public function.
void publicUnusedFn() {} /* expect: unused-code */

// Referenced within THIS file (by _useLocal) → used, not dead. dcl counts
// same-file references as usage, so this must NOT be flagged.
class OnlyLocal {}

// Never referenced anywhere, including its own file → dead public class.
class NeverReferenced {} /* expect: unused-code */

void _useLocal() {
  OnlyLocal();
  _PrivateClass();
}

// Referenced only from a string interpolation in THIS file → still used. The
// lexer folds interpolated identifiers into the string token, so the rule must
// look inside `${...}`; otherwise this is a false positive.
class InterpolatedOnly {}

String _describe() => 'value: ${InterpolatedOnly()}';
