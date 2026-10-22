#include "bv_sim_bridge/bridge_config.hh"

#include <charconv>
#include <stdexcept>
#include <string_view>

namespace bv::backend
{

BridgeConfig BridgeConfig::Parse(int argc, char **argv)
{
  BridgeConfig config;
  bool explicitAddress = false;
  for (int index = 1; index < argc; ++index)
  {
    const std::string_view option(argv[index]);
    if (option == "--healthcheck")
    {
      config.healthcheck = true;
      continue;
    }
    if (index + 1 == argc)
    {
      throw std::invalid_argument("missing value for " + std::string(option));
    }
    const std::string value(argv[++index]);
    if (value.empty())
    {
      throw std::invalid_argument("empty value for " + std::string(option));
    }
    if (option == "--address")
    {
      config.listenAddress = value;
      explicitAddress = true;
    }
    else if (option == "--port")
    {
      unsigned int port = 0;
      const auto result = std::from_chars(value.data(), value.data() + value.size(), port);
      if (result.ec != std::errc{} || result.ptr != value.data() + value.size() ||
          port == 0 || port > 65535)
      {
        throw std::invalid_argument("port must be an integer from 1 to 65535");
      }
      config.listenPort = static_cast<std::uint16_t>(port);
    }
    else if (option == "--pose-topic")
    {
      config.poseTopic = value;
    }
    else if (option == "--vehicle-name")
    {
      config.vehicleName = value;
    }
    else if (option == "--camera-name")
    {
      config.cameraName = value;
    }
    else
    {
      throw std::invalid_argument("unknown option: " + std::string(option));
    }
  }
  if (config.healthcheck && !explicitAddress)
  {
    config.listenAddress = "127.0.0.1";
  }
  if (config.vehicleName == config.cameraName)
  {
    throw std::invalid_argument("vehicle and camera must map to different Gazebo entities");
  }
  return config;
}

}
