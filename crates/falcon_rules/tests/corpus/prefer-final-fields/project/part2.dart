part of 'owner2.dart';

// A sibling that only reads `_label` — reads are not writes, so the field stays
// a final-able candidate.
String describe(Labeled x) => x._label;
