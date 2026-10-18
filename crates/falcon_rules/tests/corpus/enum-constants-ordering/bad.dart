enum Fruit { zebra, apple } /* expect: enum-constants-ordering */
enum Color { red, blue } /* expect: enum-constants-ordering */
enum Enhanced { zebra(1), apple(2); const Enhanced(this.value); final int value; } /* expect: enum-constants-ordering */
enum Named { zebra.x(), apple.x(); const Named.x(); } /* expect: enum-constants-ordering */
enum Middle { apple, zebra, berry } /* expect: enum-constants-ordering */
