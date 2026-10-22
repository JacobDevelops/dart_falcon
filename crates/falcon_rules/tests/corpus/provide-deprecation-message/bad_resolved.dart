import 'dart:core' as core;

@core.deprecated /* expect: provide-deprecation-message */
void first() {}

@core.Deprecated('') /* expect: provide-deprecation-message */
void second() {}
