int readCount() => 3;
const topConstant = 3;
final topFinal = readCount();
class Box {
  Box();
  Box.named();
  final field = 3;
  static const constant = 3;
  int typedField = 3;
}
void main() {
  int number = 3;
  String text = 'hello';
  List<int> items = <int>[];
  Box box = Box();
  Box other = Box.named();
  Box explicitNew = new Box();
  dynamic unset;
  final finalValue = readCount();
  const constantValue = 3;
  int typed = readCount();
}
