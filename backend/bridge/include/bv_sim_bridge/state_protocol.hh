#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <vector>

#include "bv_sim_bridge/state_snapshot.hh"

namespace bv::backend
{

enum class ControlKind { Hello, Heartbeat };

struct ClientControl
{
  ControlKind kind;
  std::uint64_t clientId;
  std::uint64_t runId;
};

bool ValidateSnapshot(const StateSnapshot &snapshot);
std::optional<ClientControl> DecodeControl(const std::uint8_t *data, std::size_t size);
std::vector<std::uint8_t> EncodeWelcome(std::uint64_t clientId, std::uint64_t runId);
std::vector<std::uint8_t> EncodeState(
  const StateSnapshot &snapshot, std::uint64_t runId, std::uint64_t sequence);

// A local health probe uses the same verifier and semantic checks as subscribers.
bool IsCompleteState(
  const std::uint8_t *data, std::size_t size, std::uint64_t expectedRunId);
bool ProbeBackend(const char *address, std::uint16_t port);

}
