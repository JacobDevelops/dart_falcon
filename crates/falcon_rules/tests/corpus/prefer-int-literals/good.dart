const double myDouble = 8;
final anotherDouble = myDouble + 7.0e2;
const num numValue = 1.0;
double fraction = 1.5;
var inferred = 2.0;
Object object = 3.0;
double huge = 1e300;
double parenthesized = (4.0);

void takesNum(num value) {}

void takesObject(Object value) {}

num numReturn() => 5.0;

double closure() {
  var f = () {
    return 6.0;
  };
  return f();
}

void main() {
  takesNum(7.0);
  takesObject(8.0);
  var list = [1.0, 2.0];
  var nums = <num>[3.0];
  dynamic d = 9.0;
  print([list, nums, d, 1.0.toString()]);
}

class CascadeObjectContexts {
  Object value = 0;
  void operator []=(num index, Object value) {}
}

void declaredAndCascadeContexts(CascadeObjectContexts values, num? number) {
  Object promoted = 1.0;
  if (promoted is double) {
    promoted = 2.0;
  }
  if (number is double?) {
    number ??= 3.0;
  }
  values
    ..value = 4.0
    ..[5.0] = 6.0;
  double conditionContext = (7.0 == 8.0) ? 1 : 2;
}

// The official lint checks no assignment contexts, even of `double` targets.
class CascadeDoubleContexts {
  double value = 0;
  void operator []=(double index, double value) {}
}

void cascadeContexts(CascadeDoubleContexts values) {
  values
    ..value = 13.0;
  values
    ..[14.0] =
      15.0;
}

class NumericBase {
  final double pressure, elevation;
  const NumericBase({required this.pressure, this.elevation = 0});
}

class NumericChild extends NumericBase {
  const NumericChild() : super(pressure: 1.5, elevation: (2.0));
  const NumericChild.integer() : super(pressure: 2);
}

class GenericNumericBase<T> {
  final T value;
  const GenericNumericBase(this.value);
  const GenericNumericBase.named({required this.value});
}

class BroadNumericChild extends GenericNumericBase<num> {
  const BroadNumericChild() : super(7.0);
  const BroadNumericChild.named() : super.named(value: 8.0);
}

class UnconstrainedNumericChild<T> extends GenericNumericBase<T> {
  const UnconstrainedNumericChild(T value) : super(value);
}
