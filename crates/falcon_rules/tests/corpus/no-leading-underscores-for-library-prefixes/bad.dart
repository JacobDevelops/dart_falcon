// Import prefixes must not start with an underscore.

import 'dart:math' as _math; /* expect: no-leading-underscores-for-library-prefixes */
import 'dart:async' as _async; /* expect: no-leading-underscores-for-library-prefixes */
import 'dart:convert' as _convert; /* expect: no-leading-underscores-for-library-prefixes */
import 'dart:io' as _io; /* expect: no-leading-underscores-for-library-prefixes */
import 'dart:collection' as _collection; /* expect: no-leading-underscores-for-library-prefixes */
import 'dart:typed_data' as _typed_data; /* expect: no-leading-underscores-for-library-prefixes */
