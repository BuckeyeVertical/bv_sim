#include <array>
#include <arpa/inet.h>
#include <chrono>
#include <optional>
#include <poll.h>
#include <sys/socket.h>
#include <thread>
#include <unistd.h>

#include "bv_sim_bridge/udp_state_server.hh"
#include "test_helpers.hh"

using namespace std::chrono_literals;
using namespace bv::backend;
using namespace bv::backend::test;

namespace
{

struct Received
{
  bv::sim::MessageKind kind;
  std::uint64_t clientId;
  std::uint64_t runId;
  std::uint64_t sequence;
  std::uint64_t simTime;
};

class Client
{
public:
  explicit Client(std::uint16_t port)
  {
    socket_ = socket(AF_INET, SOCK_DGRAM, 0);
    Expect(socket_ >= 0, "cannot create subscriber socket");
    sockaddr_in server{};
    server.sin_family = AF_INET;
    server.sin_port = htons(port);
    inet_pton(AF_INET, "127.0.0.1", &server.sin_addr);
    Expect(connect(socket_, reinterpret_cast<sockaddr *>(&server), sizeof(server)) == 0,
      "cannot connect subscriber socket");
  }
  ~Client() { close(socket_); }

  void Send(const std::vector<std::uint8_t> &packet)
  {
    Expect(send(socket_, packet.data(), packet.size(), 0) == static_cast<ssize_t>(packet.size()),
      "cannot send client datagram");
  }

  std::optional<Received> Read(int timeoutMs = 1000)
  {
    pollfd descriptor{socket_, POLLIN, 0};
    if (poll(&descriptor, 1, timeoutMs) <= 0) { return std::nullopt; }
    alignas(8) std::array<std::uint8_t, kMaxDatagramBytes + 1> bytes{};
    const auto size = recv(socket_, bytes.data(), bytes.size(), 0);
    Expect(size > 0, "empty response");
    flatbuffers::Verifier verifier(bytes.data(), static_cast<std::size_t>(size));
    Expect(bv::sim::VerifyPacketBuffer(verifier), "invalid FlatBuffers response");
    const auto *packet = bv::sim::GetPacket(bytes.data());
    Expect(packet->protocol_version() == 2, "wrong response version");
    if (packet->kind() == bv::sim::MessageKind::State)
    {
      Expect(IsCompleteState(bytes.data(), static_cast<std::size_t>(size), packet->run_id()),
        "partial or invalid state response");
    }
    return Received{packet->kind(), packet->client_id(), packet->run_id(),
      packet->sequence(), packet->sim_time_ns()};
  }

  Received Kind(bv::sim::MessageKind kind, std::optional<std::uint64_t> time = std::nullopt)
  {
    const auto deadline = std::chrono::steady_clock::now() + 2s;
    while (std::chrono::steady_clock::now() < deadline)
    {
      const auto packet = Read(100);
      if (packet && packet->kind == kind && (!time || packet->simTime == *time))
      {
        return *packet;
      }
    }
    throw std::runtime_error("timed out waiting for expected packet");
  }

  void Drain() { while (Read(0)) {} }

private:
  int socket_;
};

void SubscriptionsAndReset()
{
  UdpStateServer server("127.0.0.1", 0);
  server.Publish(Snapshot(100));
  Client first(server.Port());
  Client second(server.Port());
  Expect(!first.Read(50), "unsolicited subscriber received state");
  first.Send({1, 2, 3, 4});
  Expect(!first.Read(50), "malformed input received a response");
  auto oversized = Control(bv::sim::MessageKind::Hello, 11);
  oversized.resize(kMaxDatagramBytes + 100);
  first.Send(oversized);
  Expect(!first.Read(50), "oversized input received a response");
  first.Send(Control(bv::sim::MessageKind::Heartbeat, 11, 100));
  Expect(!first.Read(50), "unregistered heartbeat created a subscriber");

  first.Send(Control(bv::sim::MessageKind::Hello, 11));
  const auto welcome = first.Kind(bv::sim::MessageKind::Welcome);
  Expect(welcome.clientId == 11 && welcome.runId != 0, "Welcome handshake was not correlated");
  const auto initial = first.Kind(bv::sim::MessageKind::State, 100);
  Expect(initial.runId == welcome.runId && initial.sequence == 0, "initial state identity mismatch");
  second.Send(Control(bv::sim::MessageKind::Hello, 22));
  second.Kind(bv::sim::MessageKind::Welcome);
  second.Kind(bv::sim::MessageKind::State, 100);
  server.Publish(Snapshot(200));
  const auto update = first.Kind(bv::sim::MessageKind::State, 200);
  const auto other = second.Kind(bv::sim::MessageKind::State, 200);
  Expect(update.runId == initial.runId && update.sequence == 1 &&
    update.sequence == other.sequence, "subscribers received different state generations");

  // A pause still answers Hello with the latest full snapshot; health needs no clock advancement.
  Expect(ProbeBackend("127.0.0.1", server.Port()), "paused but ready backend failed health probe");
  server.Publish(Snapshot(50));
  const auto reset = first.Kind(bv::sim::MessageKind::State, 50);
  Expect(reset.runId != initial.runId && reset.sequence == 0, "clock reset retained old run identity");
  first.Send(Control(bv::sim::MessageKind::Heartbeat, 11, initial.runId));
  const auto renewed = first.Kind(bv::sim::MessageKind::Welcome);
  Expect(renewed.runId == reset.runId, "old-run heartbeat did not advertise the current run");

  first.Send(Control(bv::sim::MessageKind::Hello, 33));
  Expect(first.Kind(bv::sim::MessageKind::Welcome).clientId == 33,
    "fresh handshake nonce was not adopted");
  first.Kind(bv::sim::MessageKind::State);
  first.Drain();
  first.Send(Control(bv::sim::MessageKind::Hello, 11));
  Expect(!first.Read(50), "reordered Hello from retired handshake replaced current nonce");
  first.Send(Control(bv::sim::MessageKind::Heartbeat, 33, reset.runId));
  Expect(first.Kind(bv::sim::MessageKind::Welcome).clientId == 33,
    "reordered Hello invalidated current heartbeat");
}

void LeaseAndSubscriberLimit()
{
  UdpStateServer server("127.0.0.1", 0, 250ms, 1);
  server.Publish(Snapshot(100));
  Client first(server.Port());
  Client second(server.Port());
  first.Send(Control(bv::sim::MessageKind::Hello, 1));
  const auto welcome = first.Kind(bv::sim::MessageKind::Welcome);
  first.Kind(bv::sim::MessageKind::State);
  second.Send(Control(bv::sim::MessageKind::Hello, 2));
  Expect(!second.Read(50), "subscriber limit was ignored");
  std::this_thread::sleep_for(120ms);
  first.Send(Control(bv::sim::MessageKind::Heartbeat, 1, welcome.runId));
  first.Kind(bv::sim::MessageKind::Welcome);
  first.Kind(bv::sim::MessageKind::State);
  std::this_thread::sleep_for(120ms);
  server.Publish(Snapshot(200));
  first.Kind(bv::sim::MessageKind::State, 200);
  first.Drain();
  std::this_thread::sleep_for(350ms);
  server.Publish(Snapshot(300));
  Expect(!first.Read(50), "expired subscriber kept receiving state");
  second.Send(Control(bv::sim::MessageKind::Hello, 2));
  Expect(second.Kind(bv::sim::MessageKind::Welcome).clientId == 2,
    "expired subscription did not release capacity");
  second.Kind(bv::sim::MessageKind::State, 300);
}

void NewProcessRunIdentity()
{
  std::uint64_t firstRun;
  {
    UdpStateServer server("127.0.0.1", 0);
    Client client(server.Port());
    client.Send(Control(bv::sim::MessageKind::Hello, 1));
    firstRun = client.Kind(bv::sim::MessageKind::Welcome).runId;
  }
  UdpStateServer restarted("127.0.0.1", 0);
  Client client(restarted.Port());
  client.Send(Control(bv::sim::MessageKind::Hello, 1));
  Expect(client.Kind(bv::sim::MessageKind::Welcome).runId != firstRun,
    "new bridge process reused previous run identity");
}

}

int main()
{
  SubscriptionsAndReset();
  LeaseAndSubscriberLimit();
  NewProcessRunIdentity();
}
