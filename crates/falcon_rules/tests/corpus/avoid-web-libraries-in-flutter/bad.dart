// Bad: imports of web-only dart libraries.
import 'dart:html'; /* expect: avoid-web-libraries-in-flutter */
import 'dart:js'; /* expect: avoid-web-libraries-in-flutter */
import 'dart:js_util'; /* expect: avoid-web-libraries-in-flutter */
import 'dart:js_interop'; /* expect: avoid-web-libraries-in-flutter */
import 'dart:js_interop_unsafe'; /* expect: avoid-web-libraries-in-flutter */

void main() {}
