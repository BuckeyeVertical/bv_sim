#pragma once

#include <stdexcept>
#include <vector>

#include "bv_sim_bridge/state_protocol.hh"
#include "state_v2_generated.h"

namespace bv::backend::test
{

inline void Expect(bool condition, const char *message)
{
  if (!condition) { throw std::runtime_error(message); }
}

inline StateSnapshot Snapshot(std::uint64_t time = 1'250'000'000)
{
  return {time, {
    {kVehicleEntityId, {{12.5, -2.25, 30}, {0, 0, 0, 1}}},
    {kCameraEntityId, {{12.5, -2.25, 29.5}, {0, 0, 0, 1}}}}};
}

inline std::vector<std::uint8_t> Control(
  bv::sim::MessageKind kind, std::uint64_t clientId, std::uint64_t runId = 0,
  std::uint16_t version = kProtocolVersion, std::uint64_t sequence = 0)
{
  flatbuffers::FlatBufferBuilder builder;
  bv::sim::FinishPacketBuffer(builder,
    bv::sim::CreatePacket(builder, version, kind, clientId, runId, sequence));
  return {builder.GetBufferPointer(), builder.GetBufferPointer() + builder.GetSize()};
}

}
