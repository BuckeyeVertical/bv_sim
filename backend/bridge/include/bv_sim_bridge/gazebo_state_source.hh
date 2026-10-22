#pragma once

#include <functional>

#include <gz/msgs/pose_v.pb.h>
#include <gz/transport/Node.hh>

#include "bv_sim_bridge/bridge_config.hh"
#include "bv_sim_bridge/state_snapshot.hh"

namespace bv::backend
{

class GazeboStateSource
{
public:
  using SnapshotHandler = std::function<void(const StateSnapshot &)>;
  GazeboStateSource(BridgeConfig config, SnapshotHandler handler);
  bool Start();

private:
  void OnPose(const gz::msgs::Pose_V &message);
  BridgeConfig config_;
  SnapshotHandler handler_;
  gz::transport::Node node_;
};

}
