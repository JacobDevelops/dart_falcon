class Exported {}
const internal = 0;
const visibleForTesting = 0;
@internal
class Internal {}
@visibleForTesting
class Testing {}
class _Private {}
class OwnFileOnly {}
OwnFileOnly localUse() => OwnFileOnly();
void main() {}
class Unused {}
