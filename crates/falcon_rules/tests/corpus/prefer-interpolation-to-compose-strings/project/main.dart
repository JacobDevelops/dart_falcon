import 'api.dart';

String topLevel() => 'prefix ' + importedValue(); /* expect: prefer-interpolation-to-compose-strings */
String member(Formatter formatter) => 'prefix ' + formatter.format(); /* expect: prefer-interpolation-to-compose-strings */
