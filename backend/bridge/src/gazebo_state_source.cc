#include "bv_sim_bridge/gazebo_state_source.hh"

#include <limits>
#include <optional>
#include <utility>

#include "bv_sim_bridge/pose_graph.hh"
#include "bv_sim_bridge/state_protocol.hh"

namespace bv::backend
{
namespace
{

std::optional<std::uint64_t> StampNanoseconds(const gz::msgs::Pose_V &message)
{
  const gz::msgs::Header *header = nullptr;
  if (message.has_header() && message.header().has_stamp())
  {
    header = &message.header();
  }
  else if (message.pose_size() > 0 && message.pose(0).has_header() &&
           message.pose(0).header().has_stamp())
  {
    header = &message.pose(0).header();
  }
  if (header == nullptr || header->stamp().sec() < 0 || header->stamp().nsec() < 0 ||
      header->stamp().nsec() >= 1'000'000'000)
  {
    return std::nullopt;
  }
  const auto seconds = static_cast<std::uint64_t>(header->stamp().sec());
  const auto nanos = static_cast<std::uint64_t>(header->stamp().nsec());
  if (seconds > (std::numeric_limits<std::uint64_t>::max() - nanos) / 1'000'000'000ULL)
  {
    return std::nullopt;
  }
  return seconds * 1'000'000'000ULL + nanos;
}

EntityPose MakeEntity(std::uint32_t id, const gz::math::Pose3d &pose)
{
  return {id, {
    {pose.Pos().X(), pose.Pos().Y(), pose.Pos().Z()},
    {static_cast<float>(pose.Rot().X()), static_cast<float>(pose.Rot().Y()),
     static_cast<float>(pose.Rot().Z()), static_cast<float>(pose.Rot().W())}}};
}

}

GazeboStateSource::GazeboStateSource(BridgeConfig config, SnapshotHandler handler)
  : config_(std::move(config)), handler_(std::move(handler))
{
}

bool GazeboStateSource::Start()
{
  return node_.Subscribe(config_.poseTopic, &GazeboStateSource::OnPose, this);
}

void GazeboStateSource::OnPose(const gz::msgs::Pose_V &message)
{
  const auto time = StampNanoseconds(message);
  if (!time)
  {
    return;
  }
  const auto poses = ResolveWorldPoses(message);
  const auto vehicle = poses.find(config_.vehicleName);
  const auto camera = poses.find(config_.cameraName);
  // Each UDP datagram is an authoritative, complete snapshot; never send startup fragments.
  if (vehicle == poses.end() || camera == poses.end())
  {
    return;
  }
  const StateSnapshot snapshot{
    *time, {MakeEntity(kVehicleEntityId, vehicle->second), MakeEntity(kCameraEntityId, camera->second)}};
  if (ValidateSnapshot(snapshot))
  {
    handler_(snapshot);
  }
}

}
