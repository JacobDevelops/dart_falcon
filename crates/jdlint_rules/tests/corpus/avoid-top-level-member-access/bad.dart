// Test cases for avoid-top-level-member-access rule
// All lines with violations should have an expectation annotation

var globalCounter = 0; /* expect: avoid-top-level-member-access */

int globalState = 5; /* expect: avoid-top-level-member-access */

List<String> globalList = []; /* expect: avoid-top-level-member-access */

Map<String, dynamic> globalMap = {}; /* expect: avoid-top-level-member-access */

String? globalString; /* expect: avoid-top-level-member-access */

void incrementCounter() {
  globalCounter++; /* expect: avoid-top-level-member-access */
  globalCounter += 1; /* expect: avoid-top-level-member-access */
}

void useGlobalState() {
  print(globalState); /* expect: avoid-top-level-member-access */
  globalState = 10; /* expect: avoid-top-level-member-access */
}

void manipulateGlobalList() {
  globalList.add('item'); /* expect: avoid-top-level-member-access */
  globalList.clear(); /* expect: avoid-top-level-member-access */
}

int getGlobalValue() {
  return globalState * 2; /* expect: avoid-top-level-member-access */
}

class Singleton {
  static int instances = 0; /* expect: avoid-top-level-member-access */
}

bool isInitialized = false; /* expect: avoid-top-level-member-access */

void checkInitialization() {
  if (!isInitialized) { /* expect: avoid-top-level-member-access */
    isInitialized = true; /* expect: avoid-top-level-member-access */
  }
}
