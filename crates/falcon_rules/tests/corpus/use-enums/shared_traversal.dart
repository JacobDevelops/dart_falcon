@DirectiveOnly._(0)
import 'dart:core';

final before = Before._(2);

class Before {
  static const first = Before._(0);
  static const second = Before._(1);
  final int value;
  const Before._(this.value);
}

class Eligible { /* expect: use-enums */
  static const first = Eligible._(0);
  static const second = Eligible._(1);
  final int value;
  const Eligible._(this.value);
}

class After {
  static const first = After._(0);
  static const second = After._(1);
  final int value;
  const After._(this.value);
}

Object nested() => (() => After._(2))();

class Inside {
  static const first = Inside._(0);
  static const second = Inside._(1);
  final int value;
  const Inside._(this.value);
  Inside create(int value) => Inside._(value);
}

class EarlierSubclass extends ParentCandidate {
  const EarlierSubclass() : super._(2);
}

class ParentCandidate {
  static const first = ParentCandidate._(0);
  static const second = ParentCandidate._(1);
  final int value;
  const ParentCandidate._(this.value);
}

class DirectiveOnly { /* expect: use-enums */
  static const first = DirectiveOnly._(0);
  static const second = DirectiveOnly._(1);
  final int value;
  const DirectiveOnly._(this.value);
}

class LastEligible { /* expect: use-enums */
  static const first = LastEligible._(0);
  static const second = LastEligible._(1);
  final int value;
  const LastEligible._(this.value);
}
