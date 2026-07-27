abstract class Base { covariant num value; }
class Child implements Base { int value = 1; }
class Other extends Base { int another = 0; }
