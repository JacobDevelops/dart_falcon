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

// Referenced only within THIS file → still dead (should be private).
class OnlyLocal {} /* expect: unused-code */

void _useLocal() {
  OnlyLocal();
  _PrivateClass();
}
