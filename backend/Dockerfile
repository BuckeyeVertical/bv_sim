# Build from the repository root: docker build -f backend/Dockerfile -t bv-sim-backend:0.1.0 .
# No platform is forced here: build natively, or choose --platform in release CI.
FROM ubuntu:24.04@sha256:561618e2c15bf2397621dd04f96926663a3b5616c189cf7e38db7e82f5c538ea AS gazebo-base

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        curl \
        gnupg \
        lsb-release \
    && curl --fail --silent --show-error \
        https://packages.osrfoundation.org/gazebo.gpg \
        --output /usr/share/keyrings/pkgs-osrf-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/pkgs-osrf-archive-keyring.gpg] http://packages.osrfoundation.org/gazebo/ubuntu-stable $(lsb_release -cs) main" \
        > /etc/apt/sources.list.d/gazebo-stable.list \
    && apt-get update \
    && apt-get install --yes --no-install-recommends \
        gz-sim8-cli \
        gz-transport13-cli \
        libgz-physics7-dartsim \
        libgz-sim8-plugins \
    && rm -rf /var/lib/apt/lists/*

RUN plugin_dir="$(find /usr/lib -type d -path '*/gz-physics-7/engine-plugins' -print -quit)" \
    && test -n "$plugin_dir" \
    && ln --symbolic \
        libgz-physics7-dartsim-plugin.so.7 \
        "$plugin_dir/libgz-physics7-dartsim-plugin.so" \
    && ln --symbolic \
        libgz-physics7-dartsim-plugin.so \
        "$plugin_dir/libgz-physics-dartsim-plugin.so"

FROM gazebo-base AS px4-build

ARG PX4_COMMIT=7584f7567f6be0aaa677e34dd8aa0e61a0dcc34e

RUN apt-get update \
    && apt-get install --yes --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

RUN git init /src/PX4-Autopilot \
    && git -C /src/PX4-Autopilot remote add origin \
        https://github.com/PX4/PX4-Autopilot.git \
    && git -C /src/PX4-Autopilot fetch --depth 1 origin "$PX4_COMMIT" \
    && git -C /src/PX4-Autopilot checkout --detach FETCH_HEAD \
    && git -C /src/PX4-Autopilot submodule update --init --recursive --depth 1

ARG PX4_GZ_MODELS_COMMIT=ccb6275d08c59e8fdb179e0de24e2bbd89469afa

RUN git -C /src/PX4-Autopilot/Tools/simulation/gz remote set-url origin \
        https://github.com/BuckeyeVertical/PX4-gazebo-models.git \
    && git -C /src/PX4-Autopilot/Tools/simulation/gz fetch --depth 1 \
        origin "$PX4_GZ_MODELS_COMMIT" \
    && git -C /src/PX4-Autopilot/Tools/simulation/gz checkout --detach FETCH_HEAD

RUN apt-get update \
    && apt-get install --yes --no-install-recommends sudo wget \
    && rm -rf /var/lib/apt/lists/*

RUN RUNS_IN_DOCKER=true \
        /src/PX4-Autopilot/Tools/setup/ubuntu.sh --no-nuttx

RUN git -C /src/PX4-Autopilot fetch --depth 1 origin tag v1.17.0-alpha1 \
    && git -C /src/PX4-Autopilot/platforms/nuttx/NuttX/nuttx \
        fetch --depth 1 origin tag nuttx-9.0.0

RUN GIT_SUBMODULES_ARE_EVIL=1 \
        make -C /src/PX4-Autopilot -j2 px4_sitl_default \
    && cmake -S /src/PX4-Autopilot/Tools/simulation/gz/plugins \
        -B /src/PX4-Autopilot/build/bv_gz_plugins \
        -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /src/PX4-Autopilot/build/bv_gz_plugins \
    && mkdir -p \
        /runtime/bin \
        /runtime/etc \
        /runtime/plugins \
        /runtime/rootfs \
    && cp -a /src/PX4-Autopilot/build/px4_sitl_default/bin/. /runtime/bin/ \
    && cp -a /src/PX4-Autopilot/build/px4_sitl_default/etc/. /runtime/etc/ \
    && cp /src/PX4-Autopilot/build/px4_sitl_default/src/modules/simulation/gz_plugins/*.so \
        /runtime/plugins/ \
    && cp /src/PX4-Autopilot/build/bv_gz_plugins/gimbal_stabilizer/*.so \
        /runtime/plugins/ \
    && strip /runtime/bin/px4 /runtime/plugins/*.so

# Both producers use the same pinned PX4/Gazebo C++ toolchain. This avoids
# downloading and installing a second copy of Gazebo development packages.
FROM px4-build AS bridge-build

# Compile the tiny init from pinned source with that existing toolchain. Only the
# static executable and its license enter the final image; no runtime apt step.
RUN curl --fail --location --silent --show-error \
        https://codeload.github.com/krallin/tini/tar.gz/refs/tags/v0.19.0 \
        --output /tmp/tini.tar.gz \
    && echo "0fd35a7030052acd9f58948d1d900fe1e432ee37103c5561554408bdac6bbf0d  /tmp/tini.tar.gz" | sha256sum --check \
    && mkdir -p /src/tini \
    && tar -xzf /tmp/tini.tar.gz --strip-components=1 -C /src/tini \
    && rm /tmp/tini.tar.gz \
    && cmake -S /src/tini -B /build/tini -G Ninja -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /build/tini --target tini-static --parallel 2

# Pin the headers to the same FlatBuffers release used to generate C++ and Rust.
RUN curl --fail --location --silent --show-error \
        https://codeload.github.com/google/flatbuffers/tar.gz/refs/tags/v25.2.10 \
        --output /tmp/flatbuffers.tar.gz \
    && echo "b9c2df49707c57a48fc0923d52b8c73beb72d675f9d44b2211e4569be40a7421  /tmp/flatbuffers.tar.gz" | sha256sum --check \
    && mkdir -p /src/flatbuffers \
    && tar -xzf /tmp/flatbuffers.tar.gz --strip-components=1 -C /src/flatbuffers \
    && rm /tmp/flatbuffers.tar.gz

COPY protocol/generated/cpp /src/protocol/generated/cpp
COPY protocol/fixtures /src/protocol/fixtures
COPY backend/bridge /src/bridge

RUN cmake -S /src/bridge -B /build/bridge -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DBV_FLATBUFFERS_SOURCE_DIR=/src/flatbuffers \
        -DBV_GENERATED_DIR=/src/protocol/generated/cpp \
        -DBV_FIXTURE_DIR=/src/protocol/fixtures \
    && cmake --build /build/bridge --parallel 2 \
    && ctest --test-dir /build/bridge --output-on-failure --no-tests=error

FROM px4-build AS px4-runtime

COPY backend/px4/server.config /runtime/server.config
COPY backend/px4/prepare_runtime.py backend/px4/test_prepare_runtime.py /src/runtime-export/
RUN python3 -m unittest discover -s /src/runtime-export -p 'test_*.py' \
    && python3 /src/runtime-export/prepare_runtime.py \
        /src/PX4-Autopilot/Tools/simulation/gz/models /runtime/models
COPY backend/worlds/default.sdf /runtime/worlds/default.sdf

FROM gazebo-base AS simulator

COPY --from=px4-runtime /runtime /opt/px4
COPY --from=bridge-build /build/tini/tini-static /usr/bin/tini
COPY --from=bridge-build /src/tini/LICENSE /usr/share/doc/tini/LICENSE
COPY --from=bridge-build /build/bridge/bv_sim_bridge /usr/local/bin/bv_sim_bridge
COPY --chmod=0755 backend/entrypoint.sh /usr/local/bin/bv-simulator

RUN rm -rf /usr/share/gz/gz-sim8/worlds \
    && ln --symbolic /opt/px4/etc /opt/px4/rootfs/etc
WORKDIR /opt/px4/rootfs

EXPOSE 7001/udp 14580/udp 18570/udp
HEALTHCHECK --interval=2s --timeout=3s --start-period=60s --retries=15 \
    CMD test -f /run/bv-sim-ready && bv_sim_bridge --healthcheck

ENTRYPOINT ["/usr/bin/tini", "-g", "--", "/usr/local/bin/bv-simulator"]
