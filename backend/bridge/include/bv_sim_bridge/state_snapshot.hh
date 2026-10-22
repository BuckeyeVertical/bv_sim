#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

namespace bv::backend
{

constexpr std::uint16_t kProtocolVersion = 2;
constexpr std::size_t kMaxDatagramBytes = 1200;
constexpr std::uint32_t kVehicleEntityId = 1;
constexpr std::uint32_t kCameraEntityId = 2;

struct WorldPosition
{
  double x{};
  double y{};
  double z{};
};

struct WorldOrientation
{
  float x{};
  float y{};
  float z{};
  float w{1};
};

struct WorldPose
{
  WorldPosition position;
  WorldOrientation orientation;
};

struct EntityPose
{
  std::uint32_t id{};
  WorldPose pose;
};

struct StateSnapshot
{
  std::uint64_t simTimeNs{};
  std::vector<EntityPose> entities;
};

}
