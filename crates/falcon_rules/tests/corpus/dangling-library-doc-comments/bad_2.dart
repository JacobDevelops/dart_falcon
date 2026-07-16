/// Doc comment sitting directly on an import directive. /* expect: dangling-library-doc-comments */
import 'dart:math';

num area(num r) => r * r;
