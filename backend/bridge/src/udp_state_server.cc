#include "bv_sim_bridge/udp_state_server.hh"

#include <algorithm>
#include <array>
#include <arpa/inet.h>
#include <cerrno>
#include <cstring>
#include <deque>
#include <fcntl.h>
#include <iostream>
#include <limits>
#include <poll.h>
#include <random>
#include <stdexcept>
#include <sys/socket.h>
#include <unistd.h>
#include <utility>

#include "bv_sim_bridge/state_protocol.hh"

namespace bv::backend
{
namespace
{

std::uint64_t NewRunId()
{
  std::random_device random;
  std::uint64_t result = 0;
  while (result == 0)
  {
    result = (static_cast<std::uint64_t>(random()) << 32U) | random();
  }
  return result;
}

struct Subscriber
{
  sockaddr_in endpoint;
  std::uint64_t clientId;
  std::chrono::steady_clock::time_point expires;
  std::deque<std::uint64_t> retiredClientIds;
};

bool SameEndpoint(const sockaddr_in &left, const sockaddr_in &right)
{
  return left.sin_addr.s_addr == right.sin_addr.s_addr && left.sin_port == right.sin_port;
}

void Send(int socket, const sockaddr_in &endpoint, const std::vector<std::uint8_t> &packet)
{
  if (!packet.empty())
  {
    // UDP loss and temporary backpressure are harmless: the next full snapshot replaces this one.
    (void)sendto(socket, packet.data(), packet.size(), MSG_DONTWAIT,
      reinterpret_cast<const sockaddr *>(&endpoint), sizeof(endpoint));
  }
}

}

UdpStateServer::UdpStateServer(
  std::string address, std::uint16_t port,
  std::chrono::milliseconds lease, std::size_t maxSubscribers)
  : port_(port), lease_(lease), maxSubscribers_(maxSubscribers), runId_(NewRunId())
{
  if (lease.count() <= 0 || maxSubscribers == 0)
  {
    throw std::invalid_argument("subscriber lease and limit must be positive");
  }
  socket_ = socket(AF_INET, SOCK_DGRAM, 0);
  if (socket_ < 0)
  {
    throw std::runtime_error("cannot create UDP state socket");
  }
  try
  {
    if (fcntl(socket_, F_SETFL, O_NONBLOCK) < 0)
    {
      throw std::runtime_error("cannot make state socket nonblocking");
    }
    sockaddr_in endpoint{};
    endpoint.sin_family = AF_INET;
    endpoint.sin_port = htons(port_);
    if (inet_pton(AF_INET, address.c_str(), &endpoint.sin_addr) != 1)
    {
      throw std::invalid_argument("listen address must be an IPv4 address");
    }
    if (bind(socket_, reinterpret_cast<sockaddr *>(&endpoint), sizeof(endpoint)) < 0)
    {
      throw std::runtime_error("cannot bind state socket: " + std::string(std::strerror(errno)));
    }
    socklen_t size = sizeof(endpoint);
    if (getsockname(socket_, reinterpret_cast<sockaddr *>(&endpoint), &size) < 0)
    {
      throw std::runtime_error("cannot read UDP listener port");
    }
    port_ = ntohs(endpoint.sin_port);
    int wake[2];
    if (pipe(wake) < 0)
    {
      throw std::runtime_error("cannot create state-server wake pipe");
    }
    wakeRead_ = wake[0];
    wakeWrite_ = wake[1];
    if (fcntl(wakeRead_, F_SETFL, O_NONBLOCK) < 0 ||
        fcntl(wakeWrite_, F_SETFL, O_NONBLOCK) < 0)
    {
      throw std::runtime_error("cannot make wake pipe nonblocking");
    }
    worker_ = std::thread(&UdpStateServer::Serve, this);
  }
  catch (...)
  {
    close(socket_);
    if (wakeRead_ >= 0) { close(wakeRead_); }
    if (wakeWrite_ >= 0) { close(wakeWrite_); }
    throw;
  }
  std::cout << "BV binary state listening on " << address << ':' << port_ << "/udp" << std::endl;
}

UdpStateServer::~UdpStateServer()
{
  stopping_.store(true);
  Wake();
  if (worker_.joinable())
  {
    worker_.join();
  }
  close(wakeRead_);
  close(wakeWrite_);
  close(socket_);
}

std::uint16_t UdpStateServer::Port() const
{
  return port_;
}

void UdpStateServer::Wake()
{
  const char value = 1;
  ssize_t written;
  do { written = write(wakeWrite_, &value, 1); } while (written < 0 && errno == EINTR);
  if (written < 0 && errno != EAGAIN && errno != EWOULDBLOCK)
  {
    std::cerr << "Cannot wake UDP state worker: " << std::strerror(errno) << std::endl;
  }
}

void UdpStateServer::Publish(const StateSnapshot &snapshot)
{
  if (!ValidateSnapshot(snapshot))
  {
    throw std::invalid_argument("refusing an incomplete or invalid state snapshot");
  }
  {
    std::lock_guard lock(mutex_);
    if ((lastSimTime_ && snapshot.simTimeNs < *lastSimTime_) ||
        sequence_ == std::numeric_limits<std::uint64_t>::max())
    {
      const auto previousRun = runId_;
      do { runId_ = NewRunId(); } while (runId_ == previousRun);
      sequence_ = 0;
      std::cout << "Simulation clock reset; starting a new state run" << std::endl;
    }
    latest_ = EncodeState(snapshot, runId_, sequence_++);
    lastSimTime_ = snapshot.simTimeNs;
    ++generation_;
  }
  Wake();
}

void UdpStateServer::Serve()
{
  std::vector<Subscriber> subscribers;
  std::uint64_t sentGeneration = 0;
  alignas(8) std::array<std::uint8_t, kMaxDatagramBytes + 1> bytes{};
  while (!stopping_.load())
  {
    pollfd descriptors[]{{socket_, POLLIN, 0}, {wakeRead_, POLLIN, 0}};
    const int result = poll(descriptors, 2, 50);
    if (result < 0 && errno != EINTR)
    {
      std::cerr << "UDP state poll failed: " << std::strerror(errno) << std::endl;
      return;
    }
    if (descriptors[1].revents & POLLIN)
    {
      std::array<char, 256> wake{};
      while (read(wakeRead_, wake.data(), wake.size()) > 0) {}
    }

    const auto now = std::chrono::steady_clock::now();
    subscribers.erase(std::remove_if(subscribers.begin(), subscribers.end(),
      [now](const Subscriber &subscriber) { return subscriber.expires <= now; }), subscribers.end());

    std::vector<std::uint8_t> latest;
    std::uint64_t runId;
    std::uint64_t generation;
    {
      std::lock_guard lock(mutex_);
      latest = latest_;
      runId = runId_;
      generation = generation_;
    }

    // Bound work per iteration so malformed input cannot starve state delivery or shutdown.
    for (int received = 0; received < 32 && !stopping_.load(); ++received)
    {
      sockaddr_in sender{};
      socklen_t size = sizeof(sender);
      const auto count = recvfrom(socket_, bytes.data(), bytes.size(), MSG_DONTWAIT,
        reinterpret_cast<sockaddr *>(&sender), &size);
      if (count < 0)
      {
        break;
      }
      const auto control = DecodeControl(bytes.data(), static_cast<std::size_t>(count));
      if (!control)
      {
        continue;
      }
      auto subscriber = std::find_if(subscribers.begin(), subscribers.end(),
        [&sender](const Subscriber &entry) { return SameEndpoint(sender, entry.endpoint); });
      if (control->kind == ControlKind::Hello)
      {
        if (subscriber == subscribers.end())
        {
          if (subscribers.size() >= maxSubscribers_)
          {
            continue;
          }
          subscribers.push_back({sender, control->clientId, now + lease_, {}});
          subscriber = std::prev(subscribers.end());
        }
        else if (subscriber->clientId != control->clientId)
        {
          // A delayed Hello from the previous handshake must not replace the
          // active nonce and make the current client's heartbeats ineffective.
          if (std::find(subscriber->retiredClientIds.begin(), subscriber->retiredClientIds.end(),
                control->clientId) != subscriber->retiredClientIds.end())
          {
            continue;
          }
          subscriber->retiredClientIds.push_back(subscriber->clientId);
          if (subscriber->retiredClientIds.size() > 8)
          {
            subscriber->retiredClientIds.pop_front();
          }
          subscriber->clientId = control->clientId;
        }
      }
      else if (subscriber == subscribers.end() || subscriber->clientId != control->clientId)
      {
        continue;
      }
      subscriber->expires = now + lease_;
      Send(socket_, sender, EncodeWelcome(control->clientId, runId));
      // Also answer when paused, making state readiness independent of new physics ticks.
      Send(socket_, sender, latest);
    }
    if (generation != sentGeneration)
    {
      for (const auto &subscriber : subscribers)
      {
        Send(socket_, subscriber.endpoint, latest);
      }
      sentGeneration = generation;
    }
  }
}

}
