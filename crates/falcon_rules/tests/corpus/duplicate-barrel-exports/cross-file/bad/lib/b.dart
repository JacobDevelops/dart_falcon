export 'src/rival.dart' show Alpha; /* expect: duplicate-barrel-exports, msg: "Another `Alpha` is exported by `lib/a.dart`; importing both barrels is ambiguous." */
export 'src/rival.dart' show Beta; /* expect: duplicate-barrel-exports */
export 'src/rival.dart' show Gamma; /* expect: duplicate-barrel-exports */
export 'src/rival.dart' show Delta; /* expect: duplicate-barrel-exports */
export 'src/rival.dart' if (dart.library.io) 'src/alternate.dart' show Epsilon; /* expect: duplicate-barrel-exports */
export 'src/second.dart' show Collision; /* expect: duplicate-barrel-exports */
