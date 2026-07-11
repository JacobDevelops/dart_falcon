class BadOrdering {
  /// Instance field appears before static const
  int instanceField = 0; /* expect: class-members-ordering */

  static const String kAppName = 'MyApp';

  /// Method appears before fields
  void method() { /* expect: class-members-ordering */
    print('method');
  }

  static final String kVersion = '1.0.0';

  /// Private field after public methods
  int _privateField = 0; /* expect: class-members-ordering */

  /// Constructor after methods
  BadOrdering(); /* expect: class-members-ordering */

  /// Getter after constructor
  String get name => 'name'; /* expect: class-members-ordering */

  static var _internalConfig = <String>[]; /* expect: class-members-ordering */

  void publicMethod() {
    print('public');
  }

  /// Private method
  void _privateMethod() {
    print('private');
  }
}
