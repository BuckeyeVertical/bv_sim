//! Latest-snapshot UDP subscription with an explicit simulation-run handshake.

use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    io::{self, ErrorKind},
    net::{ToSocketAddrs, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

use bevy::prelude::*;
use bv_protocol::{MAX_DATAGRAM_BYTES, Message, Snapshot, decode, encode};

use super::{
    SimSnapshot, gazebo_pose_to_bevy,
    smoothing::{PlaybackClock, PoseHistory, update_smoothed_poses},
};

const DEFAULT_ENDPOINT: &str = "127.0.0.1:7001";
const ENDPOINT_ENV: &str = "BV_SIM_ENDPOINT";
const READ_TIMEOUT: Duration = Duration::from_millis(100);
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
const SESSION_TIMEOUT: Duration = Duration::from_secs(2);
const RECONNECT_DELAY: Duration = Duration::from_millis(500);

#[derive(Component, Debug)]
pub struct SimEntity {
    id: String,
}

impl SimEntity {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
}

pub struct SimReceiverPlugin {
    endpoint: String,
}

impl SimReceiverPlugin {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }
    pub fn describe(&self) -> String {
        self.endpoint.clone()
    }
    pub fn from_env() -> Self {
        Self::new(std::env::var(ENDPOINT_ENV).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned()))
    }
}

impl Plugin for SimReceiverPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SimReceiver::spawn(self.endpoint.clone()))
            .init_resource::<AppliedSimState>()
            .init_resource::<ReportedReceiverStatus>()
            .init_resource::<PlaybackClock>()
            .configure_sets(
                Update,
                SimUpdateSet::Smooth.after(SimUpdateSet::ApplySnapshot),
            )
            .add_systems(
                Update,
                (report_receiver_status, apply_latest_snapshot)
                    .chain()
                    .in_set(SimUpdateSet::ApplySnapshot),
            )
            .add_systems(Update, update_smoothed_poses.in_set(SimUpdateSet::Smooth));
    }
}

#[derive(SystemSet, Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum SimUpdateSet {
    ApplySnapshot,
    Smooth,
}

#[derive(Resource, Debug, Default)]
pub struct AppliedSimState {
    run_id: Option<u64>,
    stream_id: Option<String>,
    sequence: Option<u64>,
    sim_time_ns: Option<u64>,
}

impl AppliedSimState {
    pub fn stream_id(&self) -> Option<&str> {
        self.stream_id.as_deref()
    }
    pub fn sequence(&self) -> Option<u64> {
        self.sequence
    }
    pub fn sim_time_ns(&self) -> Option<u64> {
        self.sim_time_ns
    }
    fn record(&mut self, snapshot: &SimSnapshot) {
        self.run_id = Some(snapshot.run_id());
        self.stream_id = Some(snapshot.stream_id().to_owned());
        self.sequence = Some(snapshot.sequence());
        self.sim_time_ns = Some(snapshot.sim_time_ns());
    }
}

#[derive(Resource)]
struct SimReceiver {
    shared: Arc<ReceiverShared>,
    worker: Option<JoinHandle<()>>,
}

impl SimReceiver {
    fn spawn(endpoint: String) -> Self {
        let shared = Arc::new(ReceiverShared::default());
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("bv-state-udp".to_owned())
            .spawn(move || receive_forever(&endpoint, &worker_shared))
            .expect("failed to start simulator receiver thread");
        Self {
            shared,
            worker: Some(worker),
        }
    }
    fn take_latest(&self) -> Option<ReceiverUpdate> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
    fn status(&self) -> ReceiverStatus {
        self.shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Drop for SimReceiver {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
enum ReceiverUpdate {
    Reset,
    Snapshot(SimSnapshot),
}

#[derive(Default)]
struct ReceiverShared {
    latest: Mutex<Option<ReceiverUpdate>>,
    status: Mutex<ReceiverStatus>,
    stop: AtomicBool,
}

impl ReceiverShared {
    fn publish(&self, snapshot: SimSnapshot) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(ReceiverUpdate::Snapshot(snapshot));
    }
    fn reset(&self) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(ReceiverUpdate::Reset);
    }
    fn set_status(&self, status: ReceiverStatus) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = status;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum ReceiverStatus {
    #[default]
    Connecting,
    Connected,
    Disconnected(String),
    Stopped,
}
#[derive(Resource, Default)]
struct ReportedReceiverStatus(Option<ReceiverStatus>);

fn report_receiver_status(
    receiver: Res<SimReceiver>,
    mut reported: ResMut<ReportedReceiverStatus>,
) {
    let current = receiver.status();
    if reported.0.as_ref() == Some(&current) {
        return;
    }
    match &current {
        ReceiverStatus::Connecting => info!("Subscribing to simulator UDP state"),
        ReceiverStatus::Connected => info!("Simulator UDP subscription established"),
        ReceiverStatus::Disconnected(reason) => warn!("Simulator state unavailable: {reason}"),
        ReceiverStatus::Stopped => info!("Simulator state receiver stopped"),
    }
    reported.0 = Some(current);
}

fn apply_latest_snapshot(
    receiver: Res<SimReceiver>,
    mut applied: ResMut<AppliedSimState>,
    mut clock: ResMut<PlaybackClock>,
    mut entities: Query<(
        &SimEntity,
        &mut Transform,
        &mut Visibility,
        Option<&mut PoseHistory>,
    )>,
) {
    let Some(update) = receiver.take_latest() else {
        return;
    };
    let snapshot = match update {
        ReceiverUpdate::Reset => {
            *applied = AppliedSimState::default();
            clock.reset();
            for (_, _, mut visibility, history) in &mut entities {
                *visibility = Visibility::Hidden;
                if let Some(mut history) = history {
                    history.clear();
                }
            }
            return;
        }
        ReceiverUpdate::Snapshot(snapshot) => snapshot,
    };
    let run_changed = applied.run_id != Some(snapshot.run_id());
    if !run_changed
        && applied
            .sequence
            .is_some_and(|sequence| snapshot.sequence() <= sequence)
    {
        return;
    }
    if run_changed {
        clock.reset();
    }
    let sim_time_s = snapshot.sim_time_ns() as f64 * 1e-9;
    for (entity, mut transform, mut visibility, mut history) in &mut entities {
        // Run identity is authoritative even when a new run's time is greater.
        if run_changed && let Some(history) = history.as_mut() {
            history.clear();
        }
        let Some(state) = snapshot.entity(entity.id()) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *transform = gazebo_pose_to_bevy(state.position_m(), state.orientation());
        *visibility = Visibility::Visible;
        if let Some(mut history) = history {
            history.record(sim_time_s, *transform);
        }
    }
    applied.record(&snapshot);
}

/// A welcome is accepted only for the currently pending Hello nonce. Once a
/// run is established, a delayed welcome cannot switch the receiver's run.
struct Session {
    client_id: u64,
    run_id: Option<u64>,
    sequence: Option<u64>,
    sim_time_ns: Option<u64>,
    last_received: Instant,
    last_sent: Option<Instant>,
}

enum Decision {
    Ignore,
    Welcomed,
    Accept(Snapshot),
    Restart,
}

impl Session {
    fn new(now: Instant) -> Self {
        Self {
            client_id: fresh_client_id(),
            run_id: None,
            sequence: None,
            sim_time_ns: None,
            last_received: now,
            last_sent: None,
        }
    }
    fn restart(&mut self, now: Instant) {
        *self = Self::new(now);
    }
    fn timed_out(&self, now: Instant) -> bool {
        now.duration_since(self.last_received) >= SESSION_TIMEOUT
    }
    fn outgoing(&mut self, now: Instant) -> Option<Message> {
        if self
            .last_sent
            .is_some_and(|sent| now.duration_since(sent) < HEARTBEAT_INTERVAL)
        {
            return None;
        }
        self.last_sent = Some(now);
        Some(match self.run_id {
            Some(run_id) => Message::Heartbeat {
                client_id: self.client_id,
                run_id,
            },
            None => Message::Hello {
                client_id: self.client_id,
            },
        })
    }
    fn receive(&mut self, message: Message, now: Instant) -> Decision {
        match message {
            Message::Welcome { client_id, run_id }
                if self.run_id.is_none() && client_id == self.client_id =>
            {
                self.run_id = Some(run_id);
                self.last_received = now;
                self.last_sent = None;
                Decision::Welcomed
            }
            Message::State(snapshot) if self.run_id.is_some() => {
                if self.run_id != Some(snapshot.run_id) {
                    self.restart(now);
                    return Decision::Restart;
                }
                if self
                    .sequence
                    .is_some_and(|sequence| snapshot.sequence <= sequence)
                    || self
                        .sim_time_ns
                        .is_some_and(|time| snapshot.sim_time_ns < time)
                {
                    return Decision::Ignore;
                }
                self.sequence = Some(snapshot.sequence);
                self.sim_time_ns = Some(snapshot.sim_time_ns);
                self.last_received = now;
                Decision::Accept(snapshot)
            }
            _ => Decision::Ignore,
        }
    }
}

fn fresh_client_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    RandomState::new()
        .hash_one((
            SystemTime::now(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ))
        .max(1)
}

fn receive_forever(endpoint: &str, shared: &ReceiverShared) {
    while !shared.stop.load(Ordering::Relaxed) {
        shared.set_status(ReceiverStatus::Connecting);
        let result = connect(endpoint).and_then(|socket| receive_session(socket, shared));
        if let Err(error) = result
            && !shared.stop.load(Ordering::Relaxed)
        {
            shared.reset();
            shared.set_status(ReceiverStatus::Disconnected(error.to_string()));
        }
        let deadline = Instant::now() + RECONNECT_DELAY;
        while Instant::now() < deadline && !shared.stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(25));
        }
    }
    shared.set_status(ReceiverStatus::Stopped);
}

fn connect(endpoint: &str) -> io::Result<UdpSocket> {
    let mut last_error = None;
    for address in endpoint.to_socket_addrs()? {
        let bind_address = match (address.is_ipv4(), address.ip().is_loopback()) {
            (true, true) => "127.0.0.1:0",
            (true, false) => "0.0.0.0:0",
            (false, true) => "[::1]:0",
            (false, false) => "[::]:0",
        };
        let result = UdpSocket::bind(bind_address).and_then(|socket| {
            socket.connect(address)?;
            socket.set_read_timeout(Some(READ_TIMEOUT))?;
            socket.set_write_timeout(Some(READ_TIMEOUT))?;
            Ok(socket)
        });
        match result {
            Ok(socket) => return Ok(socket),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            ErrorKind::AddrNotAvailable,
            "endpoint resolved to no addresses",
        )
    }))
}

fn receive_session(socket: UdpSocket, shared: &ReceiverShared) -> io::Result<()> {
    let mut session = Session::new(Instant::now());
    // One extra byte detects oversized/truncated UDP packets before decoding.
    let mut buffer = [0u8; MAX_DATAGRAM_BYTES + 1];
    while !shared.stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if session.timed_out(now) {
            session.restart(now);
            shared.reset();
            shared.set_status(ReceiverStatus::Connecting);
        }
        if let Some(message) = session.outgoing(now) {
            let bytes = encode(&message)
                .map_err(|error| io::Error::new(ErrorKind::InvalidData, error.to_string()))?;
            socket.send(&bytes)?;
        }
        let length = match socket.recv(&mut buffer) {
            Ok(length) if length <= MAX_DATAGRAM_BYTES => length,
            Ok(_) => {
                warn!("Rejected oversized simulator datagram");
                continue;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        let message = match decode(&buffer[..length]) {
            Ok(message) => message,
            Err(error) => {
                warn!("Rejected simulator datagram: {error}");
                continue;
            }
        };
        match session.receive(message, Instant::now()) {
            Decision::Ignore => {}
            Decision::Welcomed => shared.set_status(ReceiverStatus::Connected),
            Decision::Restart => {
                shared.reset();
                shared.set_status(ReceiverStatus::Connecting);
            }
            Decision::Accept(snapshot) => match SimSnapshot::from_protocol(snapshot) {
                Ok(snapshot) => shared.publish(snapshot),
                Err(error) => warn!("Rejected simulator state: {error}"),
            },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bv_protocol::EntityPose;

    fn state(run_id: u64, sequence: u64, sim_time_ns: u64) -> Snapshot {
        Snapshot {
            run_id,
            sequence,
            sim_time_ns,
            entities: vec![EntityPose {
                id: bv_protocol::VEHICLE_ID,
                position_m: [5.0, 2.0, 10.0],
                orientation_xyzw: [0.0, 0.0, 0.0, 1.0],
            }],
        }
    }

    fn establish(session: &mut Session, run_id: u64, now: Instant) {
        assert!(matches!(
            session.receive(
                Message::Welcome {
                    client_id: session.client_id,
                    run_id
                },
                now
            ),
            Decision::Welcomed
        ));
    }

    #[test]
    fn requires_a_matching_welcome_before_accepting_state() {
        let now = Instant::now();
        let mut session = Session::new(now);
        assert!(matches!(
            session.receive(Message::State(state(1, 1, 10)), now),
            Decision::Ignore
        ));
        assert!(matches!(
            session.receive(
                Message::Welcome {
                    client_id: session.client_id.wrapping_add(1),
                    run_id: 1
                },
                now
            ),
            Decision::Ignore
        ));
        establish(&mut session, 1, now);
        assert!(matches!(
            session.receive(Message::State(state(1, 1, 10)), now),
            Decision::Accept(_)
        ));
    }

    #[test]
    fn rejects_duplicates_reordering_and_backward_time() {
        let now = Instant::now();
        let mut session = Session::new(now);
        establish(&mut session, 1, now);
        assert!(matches!(
            session.receive(Message::State(state(1, 4, 40)), now),
            Decision::Accept(_)
        ));
        for (sequence, time) in [(4, 40), (3, 30), (5, 39)] {
            assert!(matches!(
                session.receive(Message::State(state(1, sequence, time)), now),
                Decision::Ignore
            ));
        }
        assert!(matches!(
            session.receive(Message::State(state(1, 5, 40)), now),
            Decision::Accept(_)
        ));
    }

    #[test]
    fn changes_runs_only_after_a_fresh_matching_handshake() {
        let now = Instant::now();
        let mut session = Session::new(now);
        let original_nonce = session.client_id;
        establish(&mut session, 10, now);
        assert!(matches!(
            session.receive(Message::State(state(20, 1, 100)), now),
            Decision::Restart
        ));
        assert_ne!(session.client_id, original_nonce);
        assert!(matches!(
            session.receive(
                Message::Welcome {
                    client_id: original_nonce,
                    run_id: 10
                },
                now
            ),
            Decision::Ignore
        ));
        establish(&mut session, 20, now);
        assert!(matches!(
            session.receive(
                Message::Welcome {
                    client_id: original_nonce,
                    run_id: 10
                },
                now
            ),
            Decision::Ignore
        ));
        assert_eq!(session.run_id, Some(20));
    }

    #[test]
    fn timeout_requires_a_new_nonce_and_heartbeat_renews_the_current_run() {
        let now = Instant::now();
        let mut session = Session::new(now);
        let nonce = session.client_id;
        assert!(
            matches!(session.outgoing(now), Some(Message::Hello { client_id }) if client_id == nonce)
        );
        establish(&mut session, 2, now);
        assert!(
            matches!(session.outgoing(now), Some(Message::Heartbeat { client_id, run_id: 2 }) if client_id == nonce)
        );
        assert!(session.timed_out(now + SESSION_TIMEOUT));
        session.restart(now + SESSION_TIMEOUT);
        assert_ne!(session.client_id, nonce);
        assert!(matches!(
            session.outgoing(now + SESSION_TIMEOUT),
            Some(Message::Hello { .. })
        ));
    }

    #[test]
    fn applies_exact_pose_and_clears_history_when_run_changes_forward_in_time() {
        let shared = Arc::new(ReceiverShared::default());
        let mut app = App::new();
        app.insert_resource(SimReceiver {
            shared: shared.clone(),
            worker: None,
        })
        .init_resource::<AppliedSimState>()
        .init_resource::<PlaybackClock>()
        .add_systems(Update, apply_latest_snapshot);
        let entity = app
            .world_mut()
            .spawn((
                SimEntity::new(super::super::VEHICLE_ID),
                Transform::default(),
                Visibility::Hidden,
                PoseHistory::default(),
            ))
            .id();
        shared.publish(SimSnapshot::from_protocol(state(1, 9, 1_000_000_000)).unwrap());
        app.update();
        let mut newer = state(2, 1, 100_000_000_000);
        newer.entities[0].position_m = [100.0, 0.0, 20.0];
        shared.publish(SimSnapshot::from_protocol(newer).unwrap());
        app.update();
        let world = app.world();
        let vehicle = world.entity(entity);
        let pose = vehicle.get::<Transform>().unwrap();
        assert!(
            pose.translation
                .abs_diff_eq(Vec3::new(0.0, 20.0, -100.0), 1e-5)
        );
        assert_eq!(*vehicle.get::<Visibility>().unwrap(), Visibility::Visible);
        // An old-time sample must hold the new run's first pose, never the old run.
        assert_eq!(
            vehicle
                .get::<PoseHistory>()
                .unwrap()
                .sample(0.0)
                .unwrap()
                .translation,
            pose.translation
        );
        assert_eq!(
            world.resource::<AppliedSimState>().stream_id(),
            Some("0000000000000002")
        );
    }
}
