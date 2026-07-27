// Good: createState only returns a new State instance (or the class is not a StatefulWidget).
import 'package:flutter/material.dart';

class G1 extends StatefulWidget {
  @override
  State<G1> createState() => _G1State();
}

class G2 extends StatefulWidget {
  @override
  State<G2> createState() {
    return _G2State();
  }
}

class G3 extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final label = compute();
    return Text(label);
  }
}

class G4 {
  int createState() {
    final value = compute();
    return value;
  }
}

class G5 extends StatefulWidget {
  @override
  State<G5> createState() => _G5State();
}

class _G1State extends State<G1> {}
class _G2State extends State<G2> {}
class _G5State extends State<G5> {}
