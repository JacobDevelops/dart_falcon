// @dart = 3.13

class Named.named(int value); /* expect: public-member-api-docs */ /* expect: public-member-api-docs */
class Unnamed(); /* expect: public-member-api-docs */

/// Documents the type, not its primary constructor.
@deprecated
class Documented(); /* expect: public-member-api-docs */

/// Header fields are not separate field declarations checked by the rule.
class Fields._(final int value);

/// A primary body implements the header declaration, not a second constructor.
class Implemented.named() {
  this {} /* expect: public-member-api-docs */
}

/// Primary headers do not share ordinary constructors' closed-type exemption.
sealed class Closed(); /* expect: public-member-api-docs */
/// Abstract final primary header.
abstract final class Registry.named(); /* expect: public-member-api-docs */
/// Abstract interface primary header.
abstract interface class Port.named(); /* expect: public-member-api-docs */
class _Private();

/// Representation fields and constructors are not separately documented.
extension type Wrapper(int value) {}

/// Documents the mixin class, not its primary constructor.
mixin class PrimaryMixin(); /* expect: public-member-api-docs */
