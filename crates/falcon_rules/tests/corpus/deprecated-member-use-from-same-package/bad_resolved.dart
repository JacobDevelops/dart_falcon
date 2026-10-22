import 'dart:core' as core;

typedef LegacyNotice = core.Deprecated;

@LegacyNotice('Use replacement')
void oldFunction() {}

void use() {
  oldFunction(); /* expect: deprecated-member-use-from-same-package */
}
