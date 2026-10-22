#include "bv_sim_bridge/state_protocol.hh"

#include <algorithm>
#include <array>
#include <arpa/inet.h>
#include <chrono>
#include <cmath>
#include <poll.h>
#include <random>
#include <stdexcept>
#include <sys/socket.h>
#include <unistd.h>

#include "state_v2_generated.h"

namespace bv::backend
{
namespace
{

namespace wire = bv::sim;

const wire::Packet *VerifiedPacket(const std::uint8_t *data, std::size_t size)
{
  if (data == nullptr || size == 0 || size > kMaxDatagramBytes)
  {
    return nullptr;
  }
  flatbuffers::Verifier verifier(data, size);
  if (!wire::VerifyPacketBuffer(verifier))
  {
    return nullptr;
  }
  const auto *packet = wire::GetPacket(data);
  return packet->protocol_version() == kProtocolVersion ? packet : nullptr;
}

std::vector<std::uint8_t> Finish(
  flatbuffers::FlatBufferBuilder &builder, flatbuffers::Offset<wire::Packet> packet)
{
  wire::FinishPacketBuffer(builder, packet);
  if (builder.GetSize() > kMaxDatagramBytes)
  {
    throw std::length_error("state packet exceeds the 1200-byte datagram limit");
  }
  return {builder.GetBufferPointer(), builder.GetBufferPointer() + builder.GetSize()};
}

bool IsControl(const wire::Packet &packet)
{
  return packet.entities() == nullptr && packet.sequence() == 0 &&
         packet.sim_time_ns() == 0 && packet.client_id() != 0;
}

}

bool ValidateSnapshot(const StateSnapshot &snapshot)
{
  if (snapshot.entities.size() != 2)
  {
    return false;
  }
  bool vehicle = false;
  bool camera = false;
  for (const auto &entity : snapshot.entities)
  {
    if (entity.id == kVehicleEntityId && !vehicle)
    {
      vehicle = true;
    }
    else if (entity.id == kCameraEntityId && !camera)
    {
      camera = true;
    }
    else
    {
      return false;
    }
    const auto &p = entity.pose.position;
    const auto &q = entity.pose.orientation;
    const double norm = static_cast<double>(q.x) * q.x +
                        static_cast<double>(q.y) * q.y +
                        static_cast<double>(q.z) * q.z +
                        static_cast<double>(q.w) * q.w;
    if (!std::isfinite(p.x) || !std::isfinite(p.y) || !std::isfinite(p.z) ||
        !std::isfinite(norm) || norm < 1e-12)
    {
      return false;
    }
  }
  return vehicle && camera;
}

std::optional<ClientControl> DecodeControl(const std::uint8_t *data, std::size_t size)
{
  const auto *packet = VerifiedPacket(data, size);
  if (packet == nullptr || !IsControl(*packet))
  {
    return std::nullopt;
  }
  if (packet->kind() == wire::MessageKind::Hello && packet->run_id() == 0)
  {
    return ClientControl{ControlKind::Hello, packet->client_id(), 0};
  }
  if (packet->kind() == wire::MessageKind::Heartbeat && packet->run_id() != 0)
  {
    return ClientControl{ControlKind::Heartbeat, packet->client_id(), packet->run_id()};
  }
  return std::nullopt;
}

std::vector<std::uint8_t> EncodeWelcome(std::uint64_t clientId, std::uint64_t runId)
{
  if (clientId == 0 || runId == 0)
  {
    throw std::invalid_argument("Welcome requires nonzero client and run identities");
  }
  flatbuffers::FlatBufferBuilder builder;
  return Finish(builder, wire::CreatePacket(
    builder, kProtocolVersion, wire::MessageKind::Welcome, clientId, runId));
}

std::vector<std::uint8_t> EncodeState(
  const StateSnapshot &snapshot, std::uint64_t runId, std::uint64_t sequence)
{
  if (runId == 0 || !ValidateSnapshot(snapshot))
  {
    throw std::invalid_argument("state requires a run identity and complete valid entity poses");
  }
  flatbuffers::FlatBufferBuilder builder;
  std::vector<flatbuffers::Offset<wire::EntityState>> entities;
  for (const auto &entity : snapshot.entities)
  {
    const auto &p = entity.pose.position;
    const auto &q = entity.pose.orientation;
    const wire::Pose pose(
      wire::Position(p.x, p.y, p.z), wire::Orientation(q.x, q.y, q.z, q.w));
    entities.push_back(wire::CreateEntityState(builder, entity.id, &pose));
  }
  const auto poses = builder.CreateVector(entities);
  return Finish(builder, wire::CreatePacket(
    builder, kProtocolVersion, wire::MessageKind::State, 0, runId,
    sequence, snapshot.simTimeNs, poses));
}

bool IsCompleteState(
  const std::uint8_t *data, std::size_t size, std::uint64_t expectedRunId)
{
  const auto *packet = VerifiedPacket(data, size);
  if (packet == nullptr || expectedRunId == 0 || packet->run_id() != expectedRunId ||
      packet->kind() != wire::MessageKind::State || packet->client_id() != 0 ||
      packet->entities() == nullptr || packet->entities()->size() != 2)
  {
    return false;
  }
  StateSnapshot snapshot{packet->sim_time_ns(), {}};
  for (const auto *entity : *packet->entities())
  {
    if (entity == nullptr || entity->pose() == nullptr)
    {
      return false;
    }
    const auto &p = entity->pose()->position();
    const auto &q = entity->pose()->orientation();
    snapshot.entities.push_back({
      entity->id(), {{p.x(), p.y(), p.z()}, {q.x(), q.y(), q.z(), q.w()}}});
  }
  return ValidateSnapshot(snapshot);
}

bool ProbeBackend(const char *address, std::uint16_t port)
{
  const int socket = ::socket(AF_INET, SOCK_DGRAM, 0);
  if (socket < 0)
  {
    return false;
  }
  struct SocketGuard { int fd; ~SocketGuard() { close(fd); } } guard{socket};
  sockaddr_in endpoint{};
  endpoint.sin_family = AF_INET;
  endpoint.sin_port = htons(port);
  if (inet_pton(AF_INET, address, &endpoint.sin_addr) != 1 ||
      connect(socket, reinterpret_cast<sockaddr *>(&endpoint), sizeof(endpoint)) != 0)
  {
    return false;
  }
  std::random_device random;
  const auto clientId = (static_cast<std::uint64_t>(random()) << 32U) | random() | 1;
  flatbuffers::FlatBufferBuilder builder;
  const auto hello = Finish(builder, wire::CreatePacket(
    builder, kProtocolVersion, wire::MessageKind::Hello, clientId));
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
  auto nextHello = std::chrono::steady_clock::time_point::min();
  std::uint64_t runId = 0;
  alignas(8) std::array<std::uint8_t, kMaxDatagramBytes + 1> bytes{};
  while (std::chrono::steady_clock::now() < deadline)
  {
    const auto now = std::chrono::steady_clock::now();
    if (now >= nextHello)
    {
      if (send(socket, hello.data(), hello.size(), 0) < 0)
      {
        return false;
      }
      nextHello = now + std::chrono::milliseconds(250);
    }
    pollfd descriptor{socket, POLLIN, 0};
    if (poll(&descriptor, 1, 100) <= 0)
    {
      continue;
    }
    const auto size = recv(socket, bytes.data(), bytes.size(), 0);
    if (size <= 0)
    {
      continue;
    }
    const auto *packet = VerifiedPacket(bytes.data(), static_cast<std::size_t>(size));
    if (packet != nullptr && packet->kind() == wire::MessageKind::Welcome &&
        IsControl(*packet) && packet->client_id() == clientId && packet->run_id() != 0)
    {
      runId = packet->run_id();
    }
    else if (IsCompleteState(bytes.data(), static_cast<std::size_t>(size), runId))
    {
      return true;
    }
  }
  return false;
}

}
