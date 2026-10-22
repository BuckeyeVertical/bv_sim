#include <fstream>
#include <iostream>

#include "test_helpers.hh"

int main(int argc, char **argv)
{
  if (argc != 2)
  {
    std::cerr << "Usage: state_fixture OUTPUT_FILE\n";
    return 2;
  }
  const auto bytes = bv::backend::EncodeState(
    bv::backend::test::Snapshot(), 0x1122334455667788ULL, 42);
  std::ofstream output(argv[1], std::ios::binary);
  output.write(reinterpret_cast<const char *>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
  return output.good() ? 0 : 1;
}
