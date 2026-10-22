#include <atomic>
#include <chrono>
#include <csignal>
#include <exception>
#include <iostream>
#include <stdexcept>
#include <string_view>
#include <thread>

#include "bv_sim_bridge/bridge_config.hh"
#include "bv_sim_bridge/gazebo_state_source.hh"
#include "bv_sim_bridge/state_protocol.hh"
#include "bv_sim_bridge/udp_state_server.hh"

namespace
{
std::atomic_bool running{true};
void Stop(int) { running.store(false); }
}

int main(int argc, char **argv)
{
  if (argc == 2 && std::string_view(argv[1]) == "--help")
  {
    std::cout << "Usage: bv_sim_bridge [--address IPv4] [--port PORT]\n"
                 "  [--pose-topic TOPIC] [--vehicle-name NAME] [--camera-name NAME]\n"
                 "  [--healthcheck] (probe a running server; defaults to 127.0.0.1)\n";
    return 0;
  }
  try
  {
    const auto config = bv::backend::BridgeConfig::Parse(argc, argv);
    if (config.healthcheck)
    {
      return bv::backend::ProbeBackend(config.listenAddress.c_str(), config.listenPort) ? 0 : 1;
    }
    std::signal(SIGINT, Stop);
    std::signal(SIGTERM, Stop);
    bv::backend::UdpStateServer server(config.listenAddress, config.listenPort);
    bv::backend::GazeboStateSource source(config,
      [&server](const bv::backend::StateSnapshot &snapshot) { server.Publish(snapshot); });
    if (!source.Start())
    {
      throw std::runtime_error("cannot subscribe to " + config.poseTopic);
    }
    std::cout << "Subscribed to " << config.poseTopic << std::endl;
    while (running.load())
    {
      std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
  }
  catch (const std::exception &error)
  {
    std::cerr << "Bridge failed: " << error.what() << std::endl;
    return 1;
  }
  return 0;
}
