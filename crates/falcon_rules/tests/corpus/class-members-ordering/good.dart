class GoodOrdering {
  /// Static const first
  static const String kAppName = 'MyApp';
  static const int kMaxRetries = 3;

  /// Static final next
  static final String kVersion = '1.0.0';
  static final List<String> kValidDomains = ['example.com'];

  /// Static var
  static var _internalConfig = <String>[];

  /// Instance final fields
  final String name;
  final int value;

  /// Instance var fields
  String _description = '';
  int count = 0;

  /// Constructors: const first, then default, then named
  const GoodOrdering(this.name, this.value);

  GoodOrdering.empty()
      : name = '',
        value = 0;

  GoodOrdering.named({required this.name, this.value = 0});

  /// Factory constructors
  factory GoodOrdering.fromMap(Map<String, dynamic> data) {
    return GoodOrdering(
      data['name'] as String,
      data['value'] as int? ?? 0,
    );
  }

  /// Public getters/setters
  String get displayName => name;

  set description(String value) {
    _description = value;
  }

  /// Public methods
  void publicMethod() {
    print('public');
  }

  String process() {
    return 'processed';
  }

  /// Private getter
  bool get _isValid => name.isNotEmpty;

  /// Private methods
  void _privateMethod() {
    print('private');
  }

  void _initialize() {
    count = 0;
  }
}
