//! Binary simulation-state messages shared with the Gazebo bridge.
//!
//! Encoding and validation have no dependency on Bevy, Gazebo, or sockets.

use std::{collections::HashSet, error::Error, fmt};

#[rustfmt::skip]
#[allow(clippy::all, unused_imports, dead_code, mismatched_lifetime_syntaxes, unsafe_op_in_unsafe_fn)]
#[path = "../../../protocol/generated/rust/state_v2_generated.rs"]
mod generated;

pub use generated::bv::sim as wire;

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_DATAGRAM_BYTES: usize = 1200;
pub const MAX_ENTITIES: usize = 16;
pub const VEHICLE_ID: u32 = 1;
pub const CAMERA_ID: u32 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct EntityPose {
    pub id: u32,
    pub position_m: [f64; 3],
    pub orientation_xyzw: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub run_id: u64,
    pub sequence: u64,
    pub sim_time_ns: u64,
    pub entities: Vec<EntityPose>,
}

impl Snapshot {
    /// The legacy camera contract uses a string identity; keep this stable.
    pub fn stream_id(&self) -> String {
        format!("{:016x}", self.run_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Hello { client_id: u64 },
    Welcome { client_id: u64, run_id: u64 },
    State(Snapshot),
    Heartbeat { client_id: u64, run_id: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolError(String);

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ProtocolError {}

fn invalid(message: impl Into<String>) -> ProtocolError {
    ProtocolError(message.into())
}

/// Check semantic constraints in addition to FlatBuffers' structural verifier.
pub fn validate(message: &Message) -> Result<(), ProtocolError> {
    match message {
        Message::Hello { client_id } => {
            if *client_id == 0 {
                return Err(invalid("client ID must be nonzero"));
            }
        }
        Message::Welcome { client_id, run_id } | Message::Heartbeat { client_id, run_id } => {
            if *client_id == 0 || *run_id == 0 {
                return Err(invalid("client and run IDs must be nonzero"));
            }
        }
        Message::State(snapshot) => {
            if snapshot.run_id == 0 {
                return Err(invalid("run ID must be nonzero"));
            }
            if snapshot.entities.len() > MAX_ENTITIES {
                return Err(invalid("too many entities"));
            }
            let mut ids = HashSet::with_capacity(snapshot.entities.len());
            for entity in &snapshot.entities {
                if entity.id == 0 || !ids.insert(entity.id) {
                    return Err(invalid("entity IDs must be nonzero and unique"));
                }
                if !entity.position_m.iter().all(|value| value.is_finite()) {
                    return Err(invalid("position must be finite"));
                }
                if !entity
                    .orientation_xyzw
                    .iter()
                    .all(|value| value.is_finite())
                {
                    return Err(invalid("orientation must be finite"));
                }
                let norm_squared: f64 = entity
                    .orientation_xyzw
                    .iter()
                    .map(|value| f64::from(*value).powi(2))
                    .sum();
                if norm_squared < 1e-12 {
                    return Err(invalid("orientation must be a nonzero quaternion"));
                }
            }
        }
    }
    Ok(())
}

/// Encode exactly one UDP payload. Oversized complete snapshots fail visibly.
pub fn encode(message: &Message) -> Result<Vec<u8>, ProtocolError> {
    validate(message)?;
    let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(512);
    let (kind, client_id, run_id, sequence, sim_time_ns) = match message {
        Message::Hello { client_id } => (wire::MessageKind::Hello, *client_id, 0, 0, 0),
        Message::Welcome { client_id, run_id } => {
            (wire::MessageKind::Welcome, *client_id, *run_id, 0, 0)
        }
        Message::Heartbeat { client_id, run_id } => {
            (wire::MessageKind::Heartbeat, *client_id, *run_id, 0, 0)
        }
        Message::State(snapshot) => (
            wire::MessageKind::State,
            0,
            snapshot.run_id,
            snapshot.sequence,
            snapshot.sim_time_ns,
        ),
    };
    let entities = if let Message::State(snapshot) = message {
        let offsets: Vec<_> = snapshot
            .entities
            .iter()
            .map(|entity| {
                let [x, y, z] = entity.position_m;
                let [qx, qy, qz, qw] = entity.orientation_xyzw;
                let pose = wire::Pose::new(
                    &wire::Position::new(x, y, z),
                    &wire::Orientation::new(qx, qy, qz, qw),
                );
                wire::EntityState::create(
                    &mut builder,
                    &wire::EntityStateArgs {
                        id: entity.id,
                        pose: Some(&pose),
                    },
                )
            })
            .collect();
        Some(builder.create_vector(&offsets))
    } else {
        None
    };
    let packet = wire::Packet::create(
        &mut builder,
        &wire::PacketArgs {
            protocol_version: PROTOCOL_VERSION,
            kind,
            client_id,
            run_id,
            sequence,
            sim_time_ns,
            entities,
        },
    );
    wire::finish_packet_buffer(&mut builder, packet);
    let bytes = builder.finished_data();
    if bytes.len() > MAX_DATAGRAM_BYTES {
        return Err(invalid("snapshot exceeds 1200-byte datagram budget"));
    }
    Ok(bytes.to_vec())
}

/// Decode verified, bounded FlatBuffers data; never trust unchecked offsets.
pub fn decode(bytes: &[u8]) -> Result<Message, ProtocolError> {
    if bytes.len() < 8 || bytes.len() > MAX_DATAGRAM_BYTES {
        return Err(invalid("invalid datagram length"));
    }
    if !flatbuffers::buffer_has_identifier(bytes, "BVST", false) {
        return Err(invalid("invalid packet identifier"));
    }
    let packet = wire::root_as_packet(bytes)
        .map_err(|error| invalid(format!("malformed FlatBuffer: {error}")))?;
    if packet.protocol_version() != PROTOCOL_VERSION {
        return Err(invalid(format!(
            "unsupported protocol version {}",
            packet.protocol_version()
        )));
    }
    let message = match packet.kind() {
        wire::MessageKind::State => {
            if packet.client_id() != 0 {
                return Err(invalid("state packets must not contain a client ID"));
            }
            let source = packet
                .entities()
                .ok_or_else(|| invalid("missing entity vector"))?;
            if source.len() > MAX_ENTITIES {
                return Err(invalid("too many entities"));
            }
            let entities = source
                .iter()
                .map(|entity| {
                    let pose = entity
                        .pose()
                        .ok_or_else(|| invalid("missing entity pose"))?;
                    let position = pose.position();
                    let orientation = pose.orientation();
                    Ok(EntityPose {
                        id: entity.id(),
                        position_m: [position.x(), position.y(), position.z()],
                        orientation_xyzw: [
                            orientation.x(),
                            orientation.y(),
                            orientation.z(),
                            orientation.w(),
                        ],
                    })
                })
                .collect::<Result<Vec<_>, ProtocolError>>()?;
            Message::State(Snapshot {
                run_id: packet.run_id(),
                sequence: packet.sequence(),
                sim_time_ns: packet.sim_time_ns(),
                entities,
            })
        }
        kind => {
            if packet.sequence() != 0
                || packet.sim_time_ns() != 0
                || packet
                    .entities()
                    .is_some_and(|entities| !entities.is_empty())
            {
                return Err(invalid("control packet contains state fields"));
            }
            match kind {
                wire::MessageKind::Hello if packet.run_id() == 0 => Message::Hello {
                    client_id: packet.client_id(),
                },
                wire::MessageKind::Welcome => Message::Welcome {
                    client_id: packet.client_id(),
                    run_id: packet.run_id(),
                },
                wire::MessageKind::Heartbeat => Message::Heartbeat {
                    client_id: packet.client_id(),
                    run_id: packet.run_id(),
                },
                _ => return Err(invalid("unknown message kind or invalid Hello")),
            }
        }
    };
    validate(&message)?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            run_id: 0x0102_0304_0506_0708,
            sequence: 42,
            sim_time_ns: 1_250_000_000,
            entities: vec![
                EntityPose {
                    id: VEHICLE_ID,
                    position_m: [5., 2., 10.],
                    orientation_xyzw: [0., 0., 0., 1.],
                },
                EntityPose {
                    id: CAMERA_ID,
                    position_m: [5.25, 2., 9.75],
                    orientation_xyzw: [0., 0., 0., 1.],
                },
            ],
        }
    }

    #[test]
    fn all_message_kinds_round_trip() {
        for message in [
            Message::Hello { client_id: 9 },
            Message::Welcome {
                client_id: 9,
                run_id: 7,
            },
            Message::Heartbeat {
                client_id: 9,
                run_id: 7,
            },
            Message::State(snapshot()),
        ] {
            let bytes = encode(&message).unwrap();
            assert!(bytes.len() <= MAX_DATAGRAM_BYTES);
            assert_eq!(decode(&bytes).unwrap(), message);
        }
    }

    #[test]
    fn rejects_invalid_poses_ids_and_runs() {
        for invalid_kind in 0..6 {
            let mut state = snapshot();
            match invalid_kind {
                0 => state.run_id = 0,
                1 => state.entities[0].position_m[0] = f64::NAN,
                2 => state.entities[0].orientation_xyzw[0] = f32::INFINITY,
                3 => state.entities[0].orientation_xyzw = [0.; 4],
                4 => state.entities[1].id = VEHICLE_ID,
                _ => state.entities[0].id = 0,
            }
            assert!(encode(&Message::State(state)).is_err());
        }
        assert!(encode(&Message::Hello { client_id: 0 }).is_err());
    }

    #[test]
    fn rejects_truncated_oversized_and_wrong_identifier() {
        let bytes = encode(&Message::State(snapshot())).unwrap();
        assert!(decode(&bytes[..bytes.len() / 2]).is_err());
        assert!(decode(&vec![0; MAX_DATAGRAM_BYTES + 1]).is_err());
        assert!(decode(&[]).is_err());
        let mut wrong = bytes;
        wrong[4..8].copy_from_slice(b"NOPE");
        assert!(decode(&wrong).is_err());
    }

    #[test]
    fn rejects_unsupported_versions_and_unknown_message_kinds() {
        for (version, kind) in [
            (99, wire::MessageKind::Hello),
            (2, wire::MessageKind::Unknown),
        ] {
            let mut builder = flatbuffers::FlatBufferBuilder::new();
            let root = wire::Packet::create(
                &mut builder,
                &wire::PacketArgs {
                    protocol_version: version,
                    kind,
                    client_id: 9,
                    ..Default::default()
                },
            );
            wire::finish_packet_buffer(&mut builder, root);
            assert!(decode(builder.finished_data()).is_err());
        }
    }

    #[test]
    fn empty_complete_snapshot_is_valid_and_large_sets_are_bounded() {
        let mut state = snapshot();
        state.entities.clear();
        let message = Message::State(state);
        assert_eq!(decode(&encode(&message).unwrap()).unwrap(), message);
        let mut state = snapshot();
        state.entities = (1..=MAX_ENTITIES as u32 + 1)
            .map(|id| EntityPose {
                id,
                position_m: [0.; 3],
                orientation_xyzw: [0., 0., 0., 1.],
            })
            .collect();
        assert!(encode(&Message::State(state)).is_err());
    }

    #[test]
    fn stream_identity_is_stable_camera_v1_text() {
        assert_eq!(snapshot().stream_id(), "0102030405060708");
    }
}
