// @dart = 3.13
class _Private {}
class Public(int value) {}
class _Hidden(final _Private value) {}
class PrivateConstructor._(_Private value) {}
extension type PrivateRepresentation._(_Private _value) {}
sealed class Closed(_Private value) {}
abstract final class AbstractClosed(_Private value) {}
abstract interface class InterfaceClosed(_Private value) {}
sealed class HiddenField(final _Private _value) {}
