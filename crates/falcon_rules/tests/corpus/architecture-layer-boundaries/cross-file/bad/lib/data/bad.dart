import '../presentation/target.dart'; /* expect: architecture-layer-boundaries, msg: "The `data` layer must not import from the `presentation` layer." */
import 'package:architecture_fixture/application/target.dart'; /* expect: architecture-layer-boundaries, msg: "The `data` layer must not import from the `application` layer." */
