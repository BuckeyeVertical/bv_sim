#pragma once

#include <cstdint>
#include <string>

namespace bv::backend
{

struct BridgeConfig
{
  std::string poseTopic{"/model/x500_gimbal_0/pose"};
  std::string listenAddress{"0.0.0.0"};
  std::uint16_t listenPort{7001};
  std::string vehicleName{"x500_gimbal_0"};
  std::string cameraName{"x500_gimbal_0::camera_link::boundingbox_camera"};
  bool healthcheck{false};

  static BridgeConfig Parse(int argc, char **argv);
};

}
