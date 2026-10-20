import 'package:bloc/bloc.dart';
class Example extends Bloc<int, int> {
  Example() : super(0);
  final int value0 = 0; /* expect: avoid-bloc-public-fields */
  final int value1 = 1; /* expect: avoid-bloc-public-fields */
  final int value2 = 2; /* expect: avoid-bloc-public-fields */
  final int value3 = 3; /* expect: avoid-bloc-public-fields */
  final int value4 = 4; /* expect: avoid-bloc-public-fields */
}
