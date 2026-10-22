#include <limits>

#include "test_helpers.hh"

using namespace bv::backend;
using namespace bv::backend::test;

int main()
{
  const auto snapshot = Snapshot();
  Expect(ValidateSnapshot(snapshot), "valid full snapshot was rejected");
  const auto bytes = EncodeState(snapshot, 0x1122334455667788ULL, 42);
  Expect(bytes.size() <= kMaxDatagramBytes, "state exceeded UDP budget");
  Expect(IsCompleteState(bytes.data(), bytes.size(), 0x1122334455667788ULL), "state failed verification");
  Expect(!IsCompleteState(bytes.data(), bytes.size(), 123), "state from wrong run was accepted");
  Expect(!DecodeControl(bytes.data(), bytes.size()), "state was accepted as client control");
  Expect(!IsCompleteState(bytes.data(), bytes.size() / 2, 0x1122334455667788ULL), "truncated state accepted");
  const auto *packet = bv::sim::GetPacket(bytes.data());
  Expect(packet->sequence() == 42 && packet->sim_time_ns() == snapshot.simTimeNs,
    "state identity or timestamp changed");
  Expect(packet->entities()->Get(0)->pose()->position().x() == 12.5,
    "double-precision world position changed");

  auto invalid = snapshot;
  invalid.entities.pop_back();
  Expect(!ValidateSnapshot(invalid), "partial snapshot accepted");
  invalid = snapshot;
  invalid.entities[1].id = 1;
  Expect(!ValidateSnapshot(invalid), "duplicate entity identity accepted");
  invalid = snapshot;
  invalid.entities[1].pose.position.x = std::numeric_limits<double>::quiet_NaN();
  Expect(!ValidateSnapshot(invalid), "NaN position accepted");
  invalid = snapshot;
  invalid.entities[1].pose.orientation = {0, 0, 0, 0};
  Expect(!ValidateSnapshot(invalid), "zero quaternion accepted");
  invalid.entities[1].pose.orientation = {0, 0, 0, std::numeric_limits<float>::infinity()};
  Expect(!ValidateSnapshot(invalid), "infinite quaternion accepted");

  auto hello = Control(bv::sim::MessageKind::Hello, 55);
  const auto decoded = DecodeControl(hello.data(), hello.size());
  Expect(decoded && decoded->kind == ControlKind::Hello && decoded->clientId == 55,
    "valid subscription rejected");
  auto heartbeat = Control(bv::sim::MessageKind::Heartbeat, 55, 123);
  Expect(DecodeControl(heartbeat.data(), heartbeat.size()).has_value(), "valid heartbeat rejected");
  for (const auto &bad : {
      Control(bv::sim::MessageKind::Hello, 0),
      Control(bv::sim::MessageKind::Hello, 55, 1),
      Control(bv::sim::MessageKind::Hello, 55, 0, 1),
      Control(bv::sim::MessageKind::Hello, 55, 0, 2, 1),
      Control(bv::sim::MessageKind::Heartbeat, 55),
      Control(bv::sim::MessageKind::Welcome, 55, 123),
      Control(bv::sim::MessageKind::Unknown, 55)})
  {
    Expect(!DecodeControl(bad.data(), bad.size()), "invalid control packet accepted");
  }
  hello.resize(kMaxDatagramBytes + 1);
  Expect(!DecodeControl(hello.data(), hello.size()), "oversized datagram accepted");
  Expect(!DecodeControl(nullptr, 0), "empty datagram accepted");

  const auto welcome = EncodeWelcome(55, 123);
  const auto *reply = bv::sim::GetPacket(welcome.data());
  Expect(reply->client_id() == 55 && reply->run_id() == 123 && reply->entities() == nullptr,
    "Welcome did not echo handshake identity");
}
