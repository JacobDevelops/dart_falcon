void f(int value, String text) {
  final a = [1, 1]; /* expect: avoid-duplicate-collection-elements */
  final b = {'a', 'a'}; /* expect: avoid-duplicate-collection-elements */
  final c = [true, true]; /* expect: avoid-duplicate-collection-elements */
  final d = [null, null]; /* expect: avoid-duplicate-collection-elements */
  final e = [value, value]; /* expect: avoid-duplicate-collection-elements */
  final g = [text, text]; /* expect: avoid-duplicate-collection-elements */
  final h = [1, 2, 1]; /* expect: avoid-duplicate-collection-elements */
}
