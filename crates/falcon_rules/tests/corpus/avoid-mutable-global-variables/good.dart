/// A final top-level variable is not flagged (only const/final skips).
final RegExp kPattern = RegExp(r'[a-z]+');

/// A final top-level list initialised from a call is still final, so allowed.
final List<int> kComputed = List<int>.generate(3, (i) => i);

/// Const top-level variable
const List<String> kItems = <String>[];

/// Const int at top-level
const int kCount = 0;

/// Const regex pattern
const String kPatternString = r'[a-z]+';

/// Const list at top-level
const List<int> kNumbers = [1, 2, 3];

/// Const map at top-level
const Map<String, String> kConfig = {'key': 'value'};

/// Const string
const String kSharedState = '';

/// Class with mutable state
class MyClass {
  /// Instance fields can be mutable
  late String instanceState;

  /// Private instance field
  final List<String> _items = [];

  /// Const field in class (no instance state)
  static const String kClassName = 'MyClass';

  void example() {
    print(kItems);
  }
}
