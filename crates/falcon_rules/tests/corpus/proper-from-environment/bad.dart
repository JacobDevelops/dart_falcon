final apiUrl = String.fromEnvironment('API_URL'); /* expect: proper-from-environment */

final debug = bool.fromEnvironment('DEBUG'); /* expect: proper-from-environment */

final port = int.fromEnvironment('PORT'); /* expect: proper-from-environment */

String read() => String.fromEnvironment('KEY'); /* expect: proper-from-environment */

void f() {
  var mode = String.fromEnvironment('MODE'); /* expect: proper-from-environment */
  print(mode);
}
