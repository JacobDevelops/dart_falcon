import 'target.dart'
  if (dart.library.io) '../data/target.dart'; /* expect: architecture-layer-boundaries */
export 'target.dart'
  if (dart.library.html) '../presentation/target.dart'; /* expect: architecture-layer-boundaries */
