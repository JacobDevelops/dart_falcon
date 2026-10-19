const freezed = 0;
@freezed
class GeneratedBase {
  GeneratedChild? child; /* expect: avoid-referencing-subclasses */
}
class GeneratedChild extends GeneratedBase {}
const Generated = 0;
@Generated
class ExactBase {
  ExactChild? child; /* expect: avoid-referencing-subclasses */
}
class ExactChild extends ExactBase {}
