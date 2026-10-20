/* expect: prefer-commenting-analyzer-ignores */ // ignore: unused_import
import 'dart:math';

/* expect: prefer-commenting-analyzer-ignores */ // ignore_for_file: deprecated_member_use

/* expect: prefer-commenting-analyzer-ignores */ // ignore: unused_element, unused_local_variable
void example() {}

// A disconnected explanation.

/* expect: prefer-commenting-analyzer-ignores */ // ignore: unused_element
void disconnected() {}

//
/* expect: prefer-commenting-analyzer-ignores */ // ignore: unused_element
void emptyComment() {}

final value = 1; // This explains the variable, not the ignore.
/* expect: prefer-commenting-analyzer-ignores */ // ignore: unused_element
void trailingComment() {}

/* expect: prefer-commenting-analyzer-ignores */ // ignore: unused_element //
void emptyInline() {}

/* expect: prefer-commenting-analyzer-ignores */ // ignore: type=lint
void allLints() {}
