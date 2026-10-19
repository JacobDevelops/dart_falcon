export 'one.dart'; /* expect: avoid-barrel-files */
export 'two.dart'; /* expect: avoid-barrel-files */
export 'three.dart' show Three; /* expect: avoid-barrel-files */
export 'four.dart' hide Four; /* expect: avoid-barrel-files */
export 'five.dart' if (dart.library.io) 'io.dart'; /* expect: avoid-barrel-files */
