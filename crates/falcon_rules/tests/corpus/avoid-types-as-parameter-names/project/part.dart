part of 'main.dart';

class PartType {}
void sibling(PartType) {} /* expect: avoid-types-as-parameter-names */
void typedSibling(Object PartType) {}
