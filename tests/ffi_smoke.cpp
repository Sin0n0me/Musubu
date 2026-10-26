#include "ffi_smoke.hpp"
#include "musubu.h"
#include <array>
#include <cstdio>
#include <memory>

namespace {
using Engine = std::unique_ptr<MusubuEngine, decltype(&uninit)>;
using Value = std::unique_ptr<MusubuValueHandle, decltype(&value_free)>;
constexpr size_t Columns = 2;
constexpr size_t Rows = 3;
constexpr size_t Components = Columns * Rows;

bool matrix_roundtrip(MusubuEngine* engine) {
    const std::array<float, Components> input{1, 2, 3, 4, 5, 6};
    std::array<float, Components> output{};
    MusubuValueHandle* rawInput = nullptr;
    if (!value_create_matrix_f32(engine, input.data(), Columns, Rows, &rawInput)) {
        return false;
    }
    Value argument(rawInput, &value_free);
    constexpr char Name[] = "matrix";
    MusubuFunctionToken token{};
    size_t id = 0;
    if (!get_function_token(engine, Name, sizeof(Name) - 1, &token) ||
        !get_function_id(engine, Name, sizeof(Name) - 1, &id)) {
        return false;
    }
    const MusubuValueHandle* args[] = {argument.get()};
    MusubuValueHandle* rawResult = nullptr;
    if (!call_function_value_by_token(engine, &token, args, 1, &rawResult)) {
        return false;
    }
    Value result(rawResult, &value_free);
    if (!value_copy_matrix_f32(engine, result.get(), output.data(), Columns, Rows)) {
        return false;
    }
    const std::array<float, Components> expected{2, 4, 6, 8, 10, 12};
    if (output != expected) {
        return false;
    }
    // Failure must leave every output component unchanged.
    if (value_copy_matrix_f32(engine, result.get(), output.data(), Rows, Columns) ||
        output != expected) {
        return false;
    }
    rawResult = nullptr;
    if (!call_function_value_by_id(engine, id, args, 1, &rawResult)) {
        return false;
    }
    Value byId(rawResult, &value_free);
    return value_copy_matrix_f32(engine, byId.get(), output.data(), Columns, Rows) &&
        output == expected;
}

bool double_roundtrip(MusubuEngine* engine) {
    const std::array<double, Components> input{16777217.0, 2.0, 3.0, 4.0, 5.0, 6.0};
    std::array<double, Components> output{};
    MusubuValueHandle* rawInput = nullptr;
    if (!value_create_matrix_f64(engine, input.data(), Columns, Rows, &rawInput)) {
        return false;
    }
    Value argument(rawInput, &value_free);
    const MusubuValueHandle* args[] = {argument.get()};
    constexpr char Name[] = "double_matrix";
    MusubuValueHandle* rawResult = nullptr;
    if (!call_function_value(engine, Name, sizeof(Name) - 1, args, 1, &rawResult)) {
        return false;
    }
    Value result(rawResult, &value_free);
    return value_copy_matrix_f64(engine, result.get(), output.data(), Columns, Rows) &&
        output == input;
}

bool scalar_roundtrip(MusubuEngine* engine) {
    MusubuValue input{};
    input.kind = MUSUBU_I32;
    input.signed_value = 41;
    MusubuValue output{};
    constexpr char Name[] = "add";
    constexpr int64_t Expected = 42;
    return call_function(engine, Name, sizeof(Name) - 1, &input, 1, &output) &&
        output.kind == MUSUBU_I32 && output.signed_value == Expected;
}

void print_error(MusubuEngine* engine, bool compilation) {
    size_t length = 0;
    const char* message = compilation ? get_compile_error(engine, &length)
                                      : get_runtime_error(engine, &length);
    if (message != nullptr) {
        std::fwrite(message, sizeof(char), length, stderr);
    }
}
}

bool run_ffi_checks() {
    MusubuEngine* rawEngine = nullptr;
    if (!init(&rawEngine)) {
        return false;
    }
    Engine engine(rawEngine, &uninit);
    constexpr char Source[] = R"(
        fn matrix(m: mat2x3) -> mat2x3 { m * 2.0 }
        fn double_matrix(m: mat2x3f64) -> mat2x3f64 { m }
        fn add(x: i32) -> i32 { x + 1 }
    )";
    if (!compile(engine.get(), Source, sizeof(Source) - 1)) {
        print_error(engine.get(), true);
        return false;
    }
    if (!matrix_roundtrip(engine.get()) || !double_roundtrip(engine.get()) ||
        !scalar_roundtrip(engine.get())) {
        print_error(engine.get(), false);
        return false;
    }
    return true;
}

int main() {
    if (!run_ffi_checks()) {
        std::fputs("FFI smoke checks failed\n", stderr);
        return 1;
    }
    std::puts("FFI smoke checks passed");
    return 0;
}
