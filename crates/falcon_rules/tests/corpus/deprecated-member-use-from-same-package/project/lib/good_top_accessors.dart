@deprecated
int get oldRead => 0;
set oldRead(int value) {}
int get cleanRead => 0;
@deprecated
set cleanRead(int value) {}
void cleanAccesses() {
  oldRead = 1;
  print(cleanRead);
}
