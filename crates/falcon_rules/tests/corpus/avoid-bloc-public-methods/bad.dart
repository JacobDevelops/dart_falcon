import 'package:bloc/bloc.dart';
class Example extends Bloc<int, int> {
  Example() : super(0);
  void command0() {} /* expect: avoid-bloc-public-methods */
  void command1() {} /* expect: avoid-bloc-public-methods */
  void command2() {} /* expect: avoid-bloc-public-methods */
  void command3() {} /* expect: avoid-bloc-public-methods */
  void command4() {} /* expect: avoid-bloc-public-methods */
}
