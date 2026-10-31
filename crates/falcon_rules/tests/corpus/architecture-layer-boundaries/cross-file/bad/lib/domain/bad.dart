import '../presentation/target.dart'; /* expect: architecture-layer-boundaries, msg: "The `domain` layer must not import from the `presentation` layer." */
import 'package:architecture_fixture/application/target.dart'; /* expect: architecture-layer-boundaries, msg: "The `domain` layer must not import from the `application` layer." */
import '../data/target.dart'; /* expect: architecture-layer-boundaries, msg: "The `domain` layer must not import from the `data` layer." */
