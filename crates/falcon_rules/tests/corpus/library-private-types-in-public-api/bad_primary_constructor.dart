// @dart = 3.13
class _Private {}
class Public(final _Private value) {} /* expect: library-private-types-in-public-api */
class PublicNamed.named(_Private value) {} /* expect: library-private-types-in-public-api */
class PublicOptional([_Private? value]) {} /* expect: library-private-types-in-public-api */
extension type PublicRepresentation(_Private value) {} /* expect: library-private-types-in-public-api */
sealed class Closed(final _Private value) {} /* expect: library-private-types-in-public-api */
abstract final class AbstractClosed(var _Private value) {} /* expect: library-private-types-in-public-api */
abstract interface class InterfaceClosed(final _Private value) {} /* expect: library-private-types-in-public-api */
class PrivateConstructorWithField._(final _Private value) {} /* expect: library-private-types-in-public-api */
