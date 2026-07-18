// `_count` is initialized here and looks final-able in isolation, but a sibling
// part writes it, so it must NOT be flagged (library-wide write union).
part 'part.dart';

class Counter {
  int _count = 0;
  int get count => _count;
}
