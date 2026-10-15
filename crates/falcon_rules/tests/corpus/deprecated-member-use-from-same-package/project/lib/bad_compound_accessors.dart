@deprecated
int get oldRead => 0;
set oldRead(int value) {}
int get oldWrite => 0;
@deprecated
set oldWrite(int value) {}

void updateAccessors() {
  oldRead += 1; /* expect: deprecated-member-use-from-same-package */
  oldRead ??= 1; /* expect: deprecated-member-use-from-same-package */
  oldRead++; /* expect: deprecated-member-use-from-same-package */
  ++oldRead; /* expect: deprecated-member-use-from-same-package */
  oldWrite += 1; /* expect: deprecated-member-use-from-same-package */
  oldWrite = 1; /* expect: deprecated-member-use-from-same-package */
  oldRead = 1;
  oldWrite;
}
