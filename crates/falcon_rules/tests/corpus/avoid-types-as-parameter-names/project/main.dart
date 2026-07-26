library parameter_names;

import 'barrel.dart' show Visible, Callback;
import 'types.dart' as p;
import 'types.dart' hide Hidden, Visible, Callback, PrefixedOnly;
part 'part.dart';

void imported(Object Visible) {} /* expect: avoid-types-as-parameter-names */
void alias(Object Callback) {} /* expect: avoid-types-as-parameter-names */
void core(Object String) {} /* expect: avoid-types-as-parameter-names */
void hidden(Object Hidden) {}
void prefixed(Object PrefixedOnly) {}
void unrelated(Object Unimported) {}
