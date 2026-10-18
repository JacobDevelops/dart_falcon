import 'dart:io'; /* expect: banned-imports */
import 'package:flutter/material.dart' show Widget; /* expect: banned-imports */
import '../data/repository.dart'; /* expect: banned-imports */
import 'package:boundary_app/data/cache.dart'; /* expect: banned-imports */
import 'stub.dart' if (dart.library.io) 'dart:io'; /* expect: banned-imports */
import 'package:legacy/client.dart' deferred as legacy; /* expect: banned-imports */
import 'package:flutter/src/widgets/framework.dart' as widgets; /* expect: banned-imports */
import '../domain/../data/missing.dart'; /* expect: banned-imports */
import 'stub.dart'
    if (dart.library.io == 'true') 'package:flutter/io.dart' /* expect: banned-imports */
    if (dart.library.html) 'package:flutter/web.dart'; /* expect: banned-imports */

void main() {}
