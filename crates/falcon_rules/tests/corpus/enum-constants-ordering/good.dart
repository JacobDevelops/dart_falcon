enum Fruit { apple, zebra }
enum Color { blue, red }
enum Enhanced { apple(1), zebra(2); const Enhanced(this.value); final int value; }
enum Single { apple }
enum Named { apple.x(), zebra.x(); const Named.x(); }
