// Violations: boolean-literal declarations and bool-returning members whose
// names lack a valid prefix.

class Flags {
  bool active = true; /* expect: boolean_prefixes */
  bool loading = false; /* expect: boolean_prefixes */
  bool _enabled = true; /* expect: boolean_prefixes */

  // A non-override method returning bool without a valid prefix.
  bool getStatus() { /* expect: boolean_prefixes */
    return true;
  }

  // A getter returning bool without a valid prefix.
  bool get empty => !active; /* expect: boolean_prefixes */

  void example() {
    bool complete = false; /* expect: boolean_prefixes */
    print(complete);
  }
}

// A top-level function returning bool without a valid prefix.
bool validate() => true; /* expect: boolean_prefixes */
