part 'compound_accessors_part.dart';

@deprecated
int get oldRead => 0;
int get oldWrite => 0;

void updatePartAccessors() {
  oldRead += 1; /* expect: deprecated-member-use-from-same-package */
  oldWrite += 1; /* expect: deprecated-member-use-from-same-package */
  oldWrite = 1; /* expect: deprecated-member-use-from-same-package */
  oldRead = 1;
  oldWrite;
}
