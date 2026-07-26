class ImportedVoid {}

void accept<T>(T value) {}

class Box<T> {
  Box.named(T value);

  T field;
  void add(T value) {}
}

class VoidBox extends Box<void> {}
