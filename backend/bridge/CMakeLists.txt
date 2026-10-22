cmake_minimum_required(VERSION 3.22)
project(bv_sim_bridge LANGUAGES CXX)

include(CTest)
include(FetchContent)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
option(BV_WITH_GAZEBO "Build the production Gazebo state bridge" ON)
set(BV_FLATBUFFERS_SOURCE_DIR "" CACHE PATH "FlatBuffers 25.2.10 source checkout")
set(BV_GENERATED_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../protocol/generated/cpp" CACHE PATH
    "Directory containing state_v2_generated.h")
set(BV_FIXTURE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../protocol/fixtures" CACHE PATH
    "Cross-language binary protocol fixtures")

if(NOT EXISTS "${BV_GENERATED_DIR}/state_v2_generated.h")
  message(FATAL_ERROR "Missing generated state_v2_generated.h in ${BV_GENERATED_DIR}")
endif()

if(BV_FLATBUFFERS_SOURCE_DIR)
  set(flatbuffers_SOURCE_DIR "${BV_FLATBUFFERS_SOURCE_DIR}")
else()
  set(FLATBUFFERS_BUILD_TESTS OFF CACHE BOOL "" FORCE)
  set(FLATBUFFERS_BUILD_FLATC OFF CACHE BOOL "" FORCE)
  set(FLATBUFFERS_BUILD_FLATLIB OFF CACHE BOOL "" FORCE)
  set(FLATBUFFERS_INSTALL OFF CACHE BOOL "" FORCE)
  FetchContent_Declare(flatbuffers
    URL https://codeload.github.com/google/flatbuffers/tar.gz/refs/tags/v25.2.10
    URL_HASH SHA256=b9c2df49707c57a48fc0923d52b8c73beb72d675f9d44b2211e4569be40a7421)
  FetchContent_MakeAvailable(flatbuffers)
endif()

find_package(Threads REQUIRED)

add_library(bv_state STATIC src/state_protocol.cc src/udp_state_server.cc)
target_include_directories(bv_state PUBLIC include "${BV_GENERATED_DIR}"
    "${flatbuffers_SOURCE_DIR}/include")
target_link_libraries(bv_state PUBLIC Threads::Threads)
target_compile_options(bv_state PRIVATE -Wall -Wextra -Wpedantic -Werror)

if(BV_WITH_GAZEBO)
  find_package(gz-math7 REQUIRED)
  find_package(gz-msgs10 REQUIRED)
  find_package(gz-transport13 REQUIRED)
  add_executable(bv_sim_bridge
      src/main.cc src/bridge_config.cc src/gazebo_state_source.cc src/pose_graph.cc)
  target_link_libraries(bv_sim_bridge PRIVATE bv_state
      gz-math7::gz-math7 gz-msgs10::gz-msgs10 gz-transport13::gz-transport13)
  target_compile_options(bv_sim_bridge PRIVATE -Wall -Wextra -Wpedantic -Werror)

  if(BUILD_TESTING)
    add_executable(pose_graph_test src/pose_graph.cc test/pose_graph_test.cc)
    target_include_directories(pose_graph_test PRIVATE include)
    target_link_libraries(pose_graph_test PRIVATE gz-math7::gz-math7 gz-msgs10::gz-msgs10)
    target_compile_options(pose_graph_test PRIVATE -Wall -Wextra -Wpedantic -Werror)
    add_test(NAME pose_graph_test COMMAND pose_graph_test)
  endif()
endif()

if(BUILD_TESTING)
  foreach(test state_protocol udp_state_server)
    add_executable(${test}_test test/${test}_test.cc)
    target_link_libraries(${test}_test PRIVATE bv_state)
    target_compile_options(${test}_test PRIVATE -Wall -Wextra -Wpedantic -Werror)
    add_test(NAME ${test}_test COMMAND ${test}_test)
    set_tests_properties(${test}_test PROPERTIES TIMEOUT 15)
  endforeach()
  add_executable(state_fixture test/state_fixture.cc)
  target_link_libraries(state_fixture PRIVATE bv_state)
  add_executable(fixture_compatibility_test test/fixture_compatibility_test.cc)
  target_link_libraries(fixture_compatibility_test PRIVATE bv_state)
  target_compile_options(fixture_compatibility_test PRIVATE -Wall -Wextra -Wpedantic -Werror)
  foreach(language cpp rust)
    add_test(NAME ${language}_fixture_test COMMAND fixture_compatibility_test
        "${BV_FIXTURE_DIR}/state_v2_${language}.bvstate")
  endforeach()
endif()
