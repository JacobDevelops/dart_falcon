String greeting(String name, String city) {
  return 'Hello, ' + name + ' from ' + city; /* expect: prefer-interpolation-to-compose-strings */
}
String wrap(String value) => value;
String nested(String name) {
  return 'outer ' + wrap('inner ' + name); /* expect: prefer-interpolation-to-compose-strings */ /* expect: prefer-interpolation-to-compose-strings */
}
String interpolated(String name) => 'Hello $name' + '!'; /* expect: prefer-interpolation-to-compose-strings */
