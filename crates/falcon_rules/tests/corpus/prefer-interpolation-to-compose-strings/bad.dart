String greeting(String name, String city) {
  return 'Hello, ' + name + ' from ' + city; /* expect: prefer-interpolation-to-compose-strings */ /* expect: prefer-interpolation-to-compose-strings */
}
String wrap(String value) => value;
String nested(String name) {
  return 'outer ' + wrap('inner ' + name); /* expect: prefer-interpolation-to-compose-strings */ /* expect: prefer-interpolation-to-compose-strings */
}
String grouped(String name) => ('a' + name) + 'b'; /* expect: prefer-interpolation-to-compose-strings */ /* expect: prefer-interpolation-to-compose-strings */
String suffix(String name) => name + '!'; /* expect: prefer-interpolation-to-compose-strings */
String afterInterpolation(String name, String tail) => 'Hello $name' + tail; /* expect: prefer-interpolation-to-compose-strings */
