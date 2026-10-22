#include <fstream>
#include <iostream>
#include <iterator>

#include "test_helpers.hh"

using namespace bv::backend;
using namespace bv::backend::test;

int main(int argc, char **argv)
{
  if (argc != 2)
  {
    std::cerr << "Usage: fixture_compatibility_test FIXTURE_FILE\n";
    return 2;
  }
  std::ifstream input(argv[1], std::ios::binary);
  Expect(input.good(), "cannot open cross-language fixture");
  const std::vector<std::uint8_t> bytes{
    std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
  Expect(IsCompleteState(bytes.data(), bytes.size(), 0x1122334455667788ULL),
    "cross-language fixture failed semantic/structural verification");
  const auto *packet = bv::sim::GetPacket(bytes.data());
  Expect(packet->sequence() == 42 && packet->sim_time_ns() == 1'250'000'000,
    "cross-language fixture changed sequence or simulation time");
  for (const auto *entity : *packet->entities())
  {
    const auto &p = entity->pose()->position();
    const auto &q = entity->pose()->orientation();
    Expect(p.x() == 12.5 && p.y() == -2.25 &&
      p.z() == (entity->id() == kVehicleEntityId ? 30 : 29.5),
      "cross-language fixture changed double-precision position");
    Expect(q.x() == 0 && q.y() == 0 && q.z() == 0 && q.w() == 1,
      "cross-language fixture changed quaternion order");
  }
}
