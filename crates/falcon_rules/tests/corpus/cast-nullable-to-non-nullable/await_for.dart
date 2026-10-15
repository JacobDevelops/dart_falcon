import 'dart:async';

class Cursor {
  Object? get key => null;
}

// Async loop binding uses Stream<T>, not Iterable<T>, even without SDK source.
Future<void> consume(Stream<Cursor> cursors) async {
  await for (final cursor in cursors) {
    cursor.key as String; /* expect: cast-nullable-to-non-nullable */
  }
}

void synchronous(Iterable<Cursor> cursors) {
  for (final cursor in cursors) {
    cursor.key as String; /* expect: cast-nullable-to-non-nullable */
  }
}
