import 'package:foo/src/internal.dart'; /* expect: implementation-imports */
import 'package:bar/src/helper.dart'; /* expect: implementation-imports */
import 'package:baz/src/util/log.dart'; /* expect: implementation-imports */
import 'package:collection/src/list.dart'; /* expect: implementation-imports */
import 'package:http/src/client.dart'; /* expect: implementation-imports */

void main() {}
