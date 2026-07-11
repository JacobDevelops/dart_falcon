// Violations: boolean-literal declarations and bool-returning members whose
// names lack a valid prefix.

class Flags {
  bool active = true; /* expect: boolean-prefixes */
  bool loading = false; /* expect: boolean-prefixes */
  bool _enabled = true; /* expect: boolean-prefixes */

  // A non-override method returning bool without a valid prefix.
  bool getStatus() { /* expect: boolean-prefixes */
    return true;
  }

  // A getter returning bool without a valid prefix.
  bool get empty => !active; /* expect: boolean-prefixes */

  void example() {
    bool complete = false; /* expect: boolean-prefixes */
    print(complete);
  }
}

// A top-level function returning bool without a valid prefix.
bool validate() => true; /* expect: boolean-prefixes */
