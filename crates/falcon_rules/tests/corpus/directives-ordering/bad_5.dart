// Package URIs compare the package name before the rest of the path.
import 'package:http_parser/http_parser.dart';
import 'package:http/http.dart'; /* expect: directives-ordering */

// 'dart:' exports go before other exports.
export 'package:path/path.dart';
export 'dart:io'; /* expect: directives-ordering */

// 'package:' exports go before relative exports.
export 'local.dart';
export 'package:collection/collection.dart'; /* expect: directives-ordering */
