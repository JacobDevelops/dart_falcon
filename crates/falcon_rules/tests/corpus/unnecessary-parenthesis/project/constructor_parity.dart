import 'constructor_support.dart' as prefix;

typedef ImportedAlias<T> = prefix.Box<T>;

void importedConstructorParentheses() {
  (prefix.Box.named)<int>();
  (ImportedAlias.named)<int>();
  final imported = (prefix.Box.named)<int>; /* expect: unnecessary-parenthesis */
  (prefix.identity)<int>(1); /* expect: unnecessary-parenthesis */
  final importedFunction = (prefix.identity)<int>; /* expect: unnecessary-parenthesis */
}
