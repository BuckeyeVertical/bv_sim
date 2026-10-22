#!/usr/bin/env bash
set -euo pipefail

: "${PX4_HOME_LAT:?Set PX4_HOME_LAT from the selected scenario}"
: "${PX4_HOME_LON:?Set PX4_HOME_LON from the selected scenario}"
export PX4_HOME_LAT PX4_HOME_LON
export PX4_HOME_ALT="${PX4_HOME_ALT:-0}"

world_name="default"
world="/opt/px4/worlds/${world_name}.sdf"
if [[ ! -f "$world" ]]; then
    echo "Physics world is not included in this image: $world_name" >&2
    exit 2
fi

export GZ_IP="${GZ_IP:-127.0.0.1}"
export GZ_SIM_RESOURCE_PATH="/opt/px4/models:/opt/px4/worlds"
export GZ_SIM_SYSTEM_PLUGIN_PATH="/opt/px4/plugins"
export GZ_SIM_SERVER_CONFIG_PATH="/opt/px4/server.config"
export PX4_GZ_MODELS=/opt/px4/models
export PX4_GZ_WORLDS=/opt/px4/worlds
export PX4_GZ_PLUGINS=/opt/px4/plugins
export PX4_GZ_WORLD="$world_name"
export PX4_GZ_STANDALONE=1
export PX4_GZ_NO_FOLLOW=1
export PX4_SIM_MODEL=gz_x500_gimbal

# Identical defaults for local builds and downloaded releases. Every launch starts
# from a new container filesystem, so saved parameters cannot leak between runs.
export PX4_PARAM_SIM_BAT_MIN_PCT="${PX4_PARAM_SIM_BAT_MIN_PCT:-100}"
export PX4_PARAM_MPC_ACC_HOR="${PX4_PARAM_MPC_ACC_HOR:-2}"
export PX4_PARAM_MAV_0_BROADCAST="${PX4_PARAM_MAV_0_BROADCAST:-1}"
export PX4_PARAM_MAV_1_BROADCAST="${PX4_PARAM_MAV_1_BROADCAST:-1}"

rm -f /run/bv-sim-ready
pids=()
terminate() {
    trap - EXIT INT TERM
    if ((${#pids[@]})); then
        kill "${pids[@]}" 2>/dev/null || true
        wait "${pids[@]}" 2>/dev/null || true
    fi
}
trap terminate EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

configure_px4() {
    local nav_dll_action="${BV_PX4_NAV_DLL_ACT:-0}"
    for _ in {1..600}; do
        if (
            cd /opt/px4/rootfs
            /opt/px4/bin/px4-commander status >/dev/null 2>&1 || exit 1
            /opt/px4/bin/px4-param set NAV_DLL_ACT "$nav_dll_action" >/dev/null 2>&1 || exit 1
            /opt/px4/bin/px4-param compare NAV_DLL_ACT "$nav_dll_action" >/dev/null 2>&1 || exit 1
            /opt/px4/bin/px4-param save >/dev/null 2>&1
        ); then
            touch /run/bv-sim-ready
            echo "PX4 parameters configured; waiting for complete state in health check"
            return 0
        fi
        sleep 0.1
    done
    echo "Failed to configure PX4 simulation parameters within 60 seconds" >&2
    return 1
}

gz sim --force-version 8 -s -r "$world" &
gazebo_pid=$!
pids+=("$gazebo_pid")

bv_sim_bridge \
    --address "${BV_BRIDGE_ADDRESS:-0.0.0.0}" \
    --port "${BV_BRIDGE_PORT:-7001}" \
    --pose-topic "${BV_POSE_TOPIC:-/model/x500_gimbal_0/pose}" \
    --vehicle-name "${BV_VEHICLE_NAME:-x500_gimbal_0}" \
    --camera-name "${BV_CAMERA_NAME:-x500_gimbal_0::camera_link::boundingbox_camera}" &
bridge_pid=$!
pids+=("$bridge_pid")

(
    cd /opt/px4/rootfs
    exec /opt/px4/bin/px4 -d
) &
px4_pid=$!
pids+=("$px4_pid")

configure_px4 &
configure_pid=$!
pids+=("$configure_pid")

# Configuration is a supervised startup job: a failure is a failed container,
# rather than an unnoticed background warning while flight settings stay wrong.
set +e
wait -n -p exited "${pids[@]}"
status=$?
set -e
if [[ "${exited:-}" == "$configure_pid" && "$status" == 0 ]]; then
    pids=("$gazebo_pid" "$bridge_pid" "$px4_pid")
    set +e
    wait -n -p exited "${pids[@]}"
    status=$?
    set -e
fi

echo "Simulation process ${exited:-unknown} exited with status $status" >&2
# A critical process exiting successfully still means the simulation stopped.
if ((status == 0)); then status=1; fi
exit "$status"
