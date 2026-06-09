class BadOrdering {
  /// Instance field appears before static const
  int instanceField = 0; /* expect: class_members_ordering */

  static const String kAppName = 'MyApp';

  /// Method appears before fields
  void method() { /* expect: class_members_ordering */
    print('method');
  }

  static final String kVersion = '1.0.0';

  /// Private field after public methods
  int _privateField = 0; /* expect: class_members_ordering */

  /// Constructor after methods
  BadOrdering(); /* expect: class_members_ordering */

  /// Getter after constructor
  String get name => 'name'; /* expect: class_members_ordering */

  static var _internalConfig = <String>[]; /* expect: class_members_ordering */

  void publicMethod() {
    print('public');
  }

  /// Private method
  void _privateMethod() {
    print('private');
  }
}
