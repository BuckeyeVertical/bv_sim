//! Check a real Docker-published bridge without starting a GPU renderer.
use std::{
    error::Error,
    net::{ToSocketAddrs, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bv_protocol::{CAMERA_ID, MAX_DATAGRAM_BYTES, Message, VEHICLE_ID, decode, encode};

fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7001".into());
    let endpoint = endpoint
        .to_socket_addrs()?
        .next()
        .ok_or("cannot resolve endpoint")?;
    let socket = UdpSocket::bind(if endpoint.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    })?;
    socket.connect(endpoint)?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    let client_id = (SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64).max(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last_sent = Instant::now() - Duration::from_secs(1);
    let mut run_id = None;
    let mut buffer = [0_u8; MAX_DATAGRAM_BYTES + 1];
    while Instant::now() < deadline {
        if last_sent.elapsed() >= Duration::from_millis(500) {
            let message = match run_id {
                Some(run_id) => Message::Heartbeat { client_id, run_id },
                None => Message::Hello { client_id },
            };
            socket.send(&encode(&message)?)?;
            last_sent = Instant::now();
        }
        let size = match socket.recv(&mut buffer) {
            Ok(size) => size,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        match decode(&buffer[..size]) {
            Ok(Message::Welcome {
                client_id: echoed,
                run_id: current,
            }) if echoed == client_id => {
                run_id = Some(current);
            }
            Ok(Message::State(state)) if Some(state.run_id) == run_id => {
                if ![VEHICLE_ID, CAMERA_ID]
                    .iter()
                    .all(|id| state.entities.iter().any(|entity| entity.id == *id))
                {
                    continue;
                }
                println!(
                    "ready: run={} sequence={} sim_time_ns={} entities={} bytes={size}",
                    state.stream_id(),
                    state.sequence,
                    state.sim_time_ns,
                    state.entities.len()
                );
                for pose in state.entities {
                    println!(
                        "  entity={} position_m={:?} orientation_xyzw={:?}",
                        pose.id, pose.position_m, pose.orientation_xyzw
                    );
                }
                return Ok(());
            }
            _ => {}
        }
    }
    Err("timed out waiting for a matching Welcome and complete state".into())
}
