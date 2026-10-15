class Box<T> {
  Box();
  Box.named();
  factory Box.create() = Box<T>.named;
  static T identity<T>(T value) => value;
}

typedef Alias<T> = Box<T>;
typedef ListAlias<T> = List<T>;
T identity<T>(T value) => value;

void constructorParentheses() {
  (Box.new)<int>();
  (Box.named)<int>();
  (Box.create)<int>();
  (Alias.named)<int>();
  (List.filled)<int>(3, 0);
  (Future.value)<int>(1);
  (ListAlias.filled)<int>(3, 0);
  final unnamed = (Box.new)<int>; /* expect: unnecessary-parenthesis */
  final named = (Box.named)<int>; /* expect: unnecessary-parenthesis */
  final factory = (Box.create)<int>; /* expect: unnecessary-parenthesis */
  final alias = (Alias.named)<int>; /* expect: unnecessary-parenthesis */
  (Box<int>.named)(); /* expect: unnecessary-parenthesis */
  (identity)<int>(1); /* expect: unnecessary-parenthesis */
  (Box.identity)<int>(1); /* expect: unnecessary-parenthesis */
  final function = (identity)<int>; /* expect: unnecessary-parenthesis */
  final method = (Box.identity)<int>; /* expect: unnecessary-parenthesis */
  ((Box.named))<int>(); /* expect: unnecessary-parenthesis */
  ((Box.named)<int>)(); /* expect: unnecessary-parenthesis */
}
