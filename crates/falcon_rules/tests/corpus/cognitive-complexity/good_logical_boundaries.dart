bool below(bool a, bool b, bool c) => a || b || c;
bool at(bool a, bool b, bool c) => a || b && c;
bool separate(bool a, bool b, bool c, bool d) => (a && b) == (c || d);
