import 'package:hooks_riverpod/hooks_riverpod.dart' as hooks;

void useConditional(bool enabled) {
  if (enabled) {
    hooks.useState(0); /* expect: avoid-conditional-hooks */
  }
  for (var index = 0; hooks.useState(true); index++) { /* expect: avoid-conditional-hooks */
    break;
  }
  for (var index = 0; index < 1; hooks.useState(0)) { /* expect: avoid-conditional-hooks */
    index++;
  }
}
