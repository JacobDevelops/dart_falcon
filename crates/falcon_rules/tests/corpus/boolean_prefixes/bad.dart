class BooleanPrefixes {
  /// Missing boolean prefix for active
  bool active = true; /* expect: boolean_prefixes */

  /// Missing boolean prefix for valid
  bool valid; /* expect: boolean_prefixes */

  /// Missing boolean prefix for loading
  bool loading = false; /* expect: boolean_prefixes */

  /// Missing boolean prefix for enabled
  bool enabled = true; /* expect: boolean_prefixes */

  /// Missing boolean prefix for visible
  bool visible = false; /* expect: boolean_prefixes */

  /// Missing boolean prefix in method parameter
  void checkStatus(bool ready) { /* expect: boolean_prefixes */
    print(ready);
  }

  /// Missing boolean prefix in method return
  bool getStatus() { /* expect: boolean_prefixes */
    return true;
  }

  /// Missing boolean prefix for local variable
  void example() {
    bool complete = false; /* expect: boolean_prefixes */
    print(complete);
  }

  /// Missing boolean prefix in late variable
  late bool initialized; /* expect: boolean_prefixes */
}
