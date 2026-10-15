// Analyzer 3.13.5 checks the nullable element context for both yield forms.
Iterable<int?> yieldNullableIterable(Iterable<int>? values) sync* {
  yield* values!; /* expect: unnecessary-null-checks */
}
Stream<int?> yieldNullableStream(Stream<int>? values) async* {
  yield* values!; /* expect: unnecessary-null-checks */
}
