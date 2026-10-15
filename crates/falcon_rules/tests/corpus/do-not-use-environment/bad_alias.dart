// Analyzer-confirmed AdvB3 dnue_typedef regression.
typedef B = bool;
var v = B.fromEnvironment('a'); /* expect: do-not-use-environment */

bool? nullableContext = .fromEnvironment('a'); /* expect: do-not-use-environment */
