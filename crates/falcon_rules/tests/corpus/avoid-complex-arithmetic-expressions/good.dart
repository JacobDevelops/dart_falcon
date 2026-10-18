int compute(int a, int b, int c) => a + b * c;
int helper(int x) => x;
int calls(int a) => helper(a + a) + helper(a * a);

String concatenate(String a, String b, String c, String d) => a + b + c + d;
String literals() => 'a' + 'b' + 'c' + 'd';

class Vector {
  Vector operator +(Vector other) => this;
  Vector operator -(Vector other) => this;
  Vector operator *(Vector other) => this;
}

Vector overloaded(Vector a, Vector b, Vector c, Vector d) => a + b * (c - d);
dynamic unknown(dynamic a, dynamic b, dynamic c, dynamic d) => a + b + c + d;
