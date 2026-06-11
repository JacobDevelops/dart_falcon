// Lowercase names should be flagged
class foo {} /* expect: prefer-correct-type-name */

// Too short (only 2 chars)
class Ab {} /* expect: prefer-correct-type-name */

// Lowercase enum
enum color { red, green } /* expect: prefer-correct-type-name */

// Too short mixin (2 chars)
mixin mx {} /* expect: prefer-correct-type-name */

// Too short typedef (2 chars)
typedef cb = void Function(); /* expect: prefer-correct-type-name */

// Contains dollar sign
class Foo$Bar {} /* expect: prefer-correct-type-name */

// Too long (41 chars)
class VeryLongNameThatExceedsTheFortyCharacterLimit {} /* expect: prefer-correct-type-name */

// Extension with name that's too short
extension st on String {} /* expect: prefer-correct-type-name */

// ExtensionType with non-uppercase name
extension type val(int x) {} /* expect: prefer-correct-type-name */
