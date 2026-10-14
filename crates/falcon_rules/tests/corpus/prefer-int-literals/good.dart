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
