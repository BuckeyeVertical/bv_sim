//! Adapt the binary wire protocol into Bevy poses and legacy camera identifiers.

use std::{collections::HashMap, error::Error, fmt};

use bevy::math::{Quat, Vec3};
use bv_protocol::{CAMERA_ID, Snapshot, VEHICLE_ID};

pub const BV_SIM_SCHEMA: &str = "bv.sim_state";
pub const BV_SIM_VERSION: u32 = 2;
pub const GAZEBO_WORLD_FRAME: &str = "gazebo_world";

/// One complete, validated authoritative simulation update.
#[derive(Debug, Clone)]
pub struct SimSnapshot {
    run_id: u64,
    stream_id: String,
    sequence: u64,
    sim_time_ns: u64,
    entities: HashMap<String, EntityState>,
}

impl SimSnapshot {
    pub fn from_protocol(wire: Snapshot) -> Result<Self, SimStateError> {
        let mut entities = HashMap::with_capacity(wire.entities.len());
        for entity in wire.entities {
            let (id, kind) = match entity.id {
                VEHICLE_ID => (super::VEHICLE_ID, "vehicle"),
                CAMERA_ID => (super::ONBOARD_CAMERA_ID, "camera"),
                // Future senders may add entities this renderer does not draw.
                _ => continue,
            };
            // Positions remain double precision on the wire. Bevy's local
            // scene uses f32, so reject values outside its finite range.
            let position_m = Vec3::from_array(entity.position_m.map(|value| value as f32));
            let orientation = Quat::from_array(entity.orientation_xyzw);
            if !position_m.is_finite() {
                return Err(SimStateError::InvalidPosition(id));
            }
            if !orientation.is_finite()
                || !orientation.length_squared().is_finite()
                || orientation.length_squared() < 1e-12
            {
                return Err(SimStateError::InvalidOrientation(id));
            }
            let state = EntityState {
                id: id.to_owned(),
                kind,
                position_m,
                orientation: orientation.normalize(),
            };
            if entities.insert(id.to_owned(), state).is_some() {
                return Err(SimStateError::DuplicateEntity(id));
            }
        }
        Ok(Self {
            run_id: wire.run_id,
            // Keep camera-frame v1's string field stable across both languages.
            stream_id: format!("{:016x}", wire.run_id),
            sequence: wire.sequence,
            sim_time_ns: wire.sim_time_ns,
            entities,
        })
    }

    pub fn run_id(&self) -> u64 {
        self.run_id
    }
    pub fn stream_id(&self) -> &str {
        &self.stream_id
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn sim_time_ns(&self) -> u64 {
        self.sim_time_ns
    }
    pub fn frame_id(&self) -> &str {
        GAZEBO_WORLD_FRAME
    }
    pub fn entity(&self, id: &str) -> Option<&EntityState> {
        self.entities.get(id)
    }
    pub fn entities(&self) -> impl Iterator<Item = &EntityState> {
        self.entities.values()
    }
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
}

#[derive(Debug, Clone)]
pub struct EntityState {
    id: String,
    kind: &'static str,
    position_m: Vec3,
    orientation: Quat,
}

impl EntityState {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn kind(&self) -> &str {
        self.kind
    }
    pub fn position_m(&self) -> Vec3 {
        self.position_m
    }
    pub fn orientation(&self) -> Quat {
        self.orientation
    }
}

#[derive(Debug)]
pub enum SimStateError {
    DuplicateEntity(&'static str),
    InvalidPosition(&'static str),
    InvalidOrientation(&'static str),
}

impl fmt::Display for SimStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEntity(id) => write!(f, "duplicate entity {id}"),
            Self::InvalidPosition(id) => write!(f, "invalid position for {id}"),
            Self::InvalidOrientation(id) => write!(f, "invalid orientation for {id}"),
        }
    }
}
impl Error for SimStateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use bv_protocol::EntityPose;

    fn wire() -> Snapshot {
        Snapshot {
            run_id: 0xabcdef,
            sequence: 42,
            sim_time_ns: 1_250_000_000,
            entities: vec![EntityPose {
                id: VEHICLE_ID,
                position_m: [5.0, 2.0, 10.0],
                orientation_xyzw: [0.0, 0.0, 0.0, 2.0],
            }],
        }
    }

    #[test]
    fn maps_binary_ids_and_run_id_to_existing_camera_contract() {
        let snapshot = SimSnapshot::from_protocol(wire()).unwrap();
        let vehicle = snapshot.entity(super::super::VEHICLE_ID).unwrap();
        assert_eq!(snapshot.stream_id(), "0000000000abcdef");
        assert_eq!(snapshot.frame_id(), GAZEBO_WORLD_FRAME);
        assert_eq!(vehicle.position_m(), Vec3::new(5.0, 2.0, 10.0));
        assert!(vehicle.orientation().abs_diff_eq(Quat::IDENTITY, 1e-6));
    }

    #[test]
    fn additional_unknown_entities_do_not_hide_known_vehicle_and_camera() {
        let mut wire = wire();
        let mut extra = wire.entities[0].clone();
        extra.id = 99;
        wire.entities.push(extra);
        let snapshot = SimSnapshot::from_protocol(wire).unwrap();
        assert_eq!(snapshot.entity_count(), 1);
        assert!(snapshot.entity(super::super::VEHICLE_ID).is_some());
    }

    #[test]
    fn rejects_positions_that_cannot_be_represented_in_bevy() {
        let mut wire = wire();
        wire.entities[0].position_m[0] = f64::MAX;
        assert!(matches!(
            SimSnapshot::from_protocol(wire),
            Err(SimStateError::InvalidPosition(_))
        ));
    }

    #[test]
    fn rejects_duplicate_entities_and_invalid_rotations() {
        let mut duplicate = wire();
        duplicate.entities.push(duplicate.entities[0].clone());
        assert!(matches!(
            SimSnapshot::from_protocol(duplicate),
            Err(SimStateError::DuplicateEntity(_))
        ));
        let mut invalid = wire();
        invalid.entities[0].orientation_xyzw = [0.0; 4];
        assert!(matches!(
            SimSnapshot::from_protocol(invalid),
            Err(SimStateError::InvalidOrientation(_))
        ));
    }
}
