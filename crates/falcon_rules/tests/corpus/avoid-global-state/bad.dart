// Bad: mutable global state
var globalList = <String>[]; /* expect: avoid-global-state */
int requestCount = 0; /* expect: avoid-global-state */
String? cachedValue; /* expect: avoid-global-state */

Map<String, dynamic> config = {}; /* expect: avoid-global-state */

class Database {
  static var instance; /* expect: avoid-global-state */
}

List<int> shared = []; /* expect: avoid-global-state */

void incrementCounter() {
  requestCount++; // uses bad global
}

late final String lateGlobal; /* expect: avoid-global-state */
var uninitializedGlobal; /* expect: avoid-global-state */
