//! Regenerate the Rust-produced cross-language fixture after a protocol change.
use bv_protocol::{CAMERA_ID, EntityPose, Message, Snapshot, VEHICLE_ID, encode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("usage: write_fixture OUTPUT")?;
    let packet = Message::State(Snapshot {
        run_id: 0x1122_3344_5566_7788,
        sequence: 42,
        sim_time_ns: 1_250_000_000,
        entities: vec![
            EntityPose {
                id: VEHICLE_ID,
                position_m: [12.5, -2.25, 30.0],
                orientation_xyzw: [0., 0., 0., 1.],
            },
            EntityPose {
                id: CAMERA_ID,
                position_m: [12.5, -2.25, 29.5],
                orientation_xyzw: [0., 0., 0., 1.],
            },
        ],
    });
    std::fs::write(output, encode(&packet)?)?;
    Ok(())
}
