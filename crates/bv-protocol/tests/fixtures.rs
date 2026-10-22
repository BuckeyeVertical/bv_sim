use bv_protocol::{CAMERA_ID, Message, VEHICLE_ID, decode};

#[test]
fn decodes_independent_cpp_and_rust_fixtures() {
    for bytes in [
        include_bytes!("../../../protocol/fixtures/state_v2_cpp.bvstate").as_slice(),
        include_bytes!("../../../protocol/fixtures/state_v2_rust.bvstate").as_slice(),
    ] {
        let Message::State(state) = decode(bytes).unwrap() else {
            panic!("expected state")
        };
        assert_eq!(state.run_id, 0x1122_3344_5566_7788);
        assert_eq!(state.sequence, 42);
        assert_eq!(state.sim_time_ns, 1_250_000_000);
        assert_eq!(state.entities.len(), 2);
        assert_eq!(state.entities[0].id, VEHICLE_ID);
        assert_eq!(state.entities[0].position_m, [12.5, -2.25, 30.0]);
        assert_eq!(state.entities[1].id, CAMERA_ID);
        assert_eq!(state.entities[1].position_m, [12.5, -2.25, 29.5]);
        for entity in state.entities {
            assert_eq!(entity.orientation_xyzw, [0., 0., 0., 1.]);
        }
    }
}
