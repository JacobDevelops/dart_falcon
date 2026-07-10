final apiUrl = String.fromEnvironment('API_URL'); /* expect: proper_from_environment */

final debug = bool.fromEnvironment('DEBUG'); /* expect: proper_from_environment */

final port = int.fromEnvironment('PORT'); /* expect: proper_from_environment */

String read() => String.fromEnvironment('KEY'); /* expect: proper_from_environment */

void f() {
  var mode = String.fromEnvironment('MODE'); /* expect: proper_from_environment */
  print(mode);
}
