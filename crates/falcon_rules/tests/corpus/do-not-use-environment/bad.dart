import 'dart:core';
import 'dart:core' as core;

const bool debug = bool.fromEnvironment('DEBUG'); /* expect: do-not-use-environment */
const int port = int.fromEnvironment('PORT', defaultValue: 8080); /* expect: do-not-use-environment */
const String flavor = String.fromEnvironment('FLAVOR'); /* expect: do-not-use-environment */
const bool hasFlavor = bool.hasEnvironment('FLAVOR'); /* expect: do-not-use-environment */
final bool runtimeDebug = new bool.fromEnvironment('DEBUG'); /* expect: do-not-use-environment */
const prefixed = core.String.fromEnvironment('NAME'); /* expect: do-not-use-environment */
const explicitConst = const int.fromEnvironment('LEVEL'); /* expect: do-not-use-environment */
const bool shorthand = .fromEnvironment('SHORT'); /* expect: do-not-use-environment */

class Config {
  static const String apiKey = String.fromEnvironment('API_KEY'); /* expect: do-not-use-environment */
  final int retries = .fromEnvironment('RETRIES'); /* expect: do-not-use-environment */
}

void main() {
  const bool local = bool.fromEnvironment('LOCAL'); /* expect: do-not-use-environment */
  print([local, debug, port, flavor, hasFlavor, runtimeDebug, prefixed, explicitConst, shorthand]);
}
