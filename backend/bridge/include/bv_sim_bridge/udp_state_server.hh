#pragma once

#include <atomic>
#include <chrono>
#include <cstdint>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <vector>

#include "bv_sim_bridge/state_snapshot.hh"

namespace bv::backend
{

class UdpStateServer
{
public:
  UdpStateServer(
    std::string address, std::uint16_t port,
    std::chrono::milliseconds lease = std::chrono::seconds(3),
    std::size_t maxSubscribers = 8);
  ~UdpStateServer();

  UdpStateServer(const UdpStateServer &) = delete;
  UdpStateServer &operator=(const UdpStateServer &) = delete;

  std::uint16_t Port() const;
  void Publish(const StateSnapshot &snapshot);

private:
  void Serve();
  void Wake();

  std::uint16_t port_;
  std::chrono::milliseconds lease_;
  std::size_t maxSubscribers_;
  int socket_{-1};
  int wakeRead_{-1};
  int wakeWrite_{-1};
  std::atomic_bool stopping_{false};
  std::mutex mutex_;
  std::uint64_t runId_;
  std::uint64_t sequence_{0};
  std::uint64_t generation_{0};
  std::optional<std::uint64_t> lastSimTime_;
  std::vector<std::uint8_t> latest_;
  std::thread worker_;
};

}
