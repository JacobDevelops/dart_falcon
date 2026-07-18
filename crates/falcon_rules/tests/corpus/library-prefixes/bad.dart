// Import prefixes must be lower_case_with_underscores.

import 'dart:math' as Math; /* expect: library-prefixes */
import 'dart:async' as MyAsync; /* expect: library-prefixes */
import 'dart:convert' as JSON; /* expect: library-prefixes */
import 'dart:io' as IO; /* expect: library-prefixes */
import 'dart:collection' as CollectionLib; /* expect: library-prefixes */
import 'dart:typed_data' as TypedData; /* expect: library-prefixes */
