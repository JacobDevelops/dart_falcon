// Standard class with UpperCamelCase
class Foo {}

// Longer descriptive name
class HttpClient {}

// Private class with underscore prefix (stripped, "Internal" is 8 chars and uppercase)
class _Internal {}

// Enum with proper case
enum Color { red, green }

// Mixin with proper case
mixin Loggable {}

// TypeAlias with proper case
typedef Callback = void Function();

// ExtensionType with proper name
extension type IntId(int value) {}

// Extension without name (should not be checked)
extension on String {}

// Extension with a proper name
extension StringX on String {}

// Edge case: exactly 3 chars (minimum)
class Foo {}

// Edge case: exactly 40 chars
class HttpClientWithVeryDetailedNameHere {}

// Private type with valid stripped name
class _HttpClientInternal {}
