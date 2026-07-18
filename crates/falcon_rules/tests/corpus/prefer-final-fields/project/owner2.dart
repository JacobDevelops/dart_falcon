// `_label` is initialized once and never written anywhere in the resolved
// library, so it should still be flagged even though the class spans a part —
// the library awareness must not over-suppress.
part 'part2.dart';

class Labeled {
  String _label = 'init'; /* expect: prefer-final-fields */
  String get label => _label;
}
