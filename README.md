# Musubu

自作アプリケーション用の組み込み言語

## MSVC

公開APIは [musubu.h](compiler/musubu/include/musubu.h) に定義されています。
呼び出し側では、このヘッダをインクルードしてください。
以前の `compile(code, length)` 宣言や `run_script()` ではなく、エンジンを作成して関数を呼び出します。

## FFIからの関数呼び出し

1. `init()` でエンジンを作成します。
2. `compile()` または `compile_with_filename()` でソースをコンパイルします。
3. `get_function_id()` でソース上の関数名からIDを取得します。
4. `call_function_value_by_id()` で呼び出し、戻り値のハンドルを取得します。
5. 値ハンドルは `value_free()`、エンジンは `uninit()` で解放します。

関数名は `"transform"` のように指定します。`main` や `extern "C"` の定義は不要です。
ソース・関数名・ファイル名はUTF-8で渡し、長さには終端NULを含まない**バイト数**を指定します。

### 関数IDの再利用

`get_function_id()` は初期化時などに一度呼び、取得した `size_t` のIDを保持します。
以降の `call_function_value_by_id()` はUTF-8解析、関数名の文字列生成、名前検索を行いません。
引数の個数・型と戻り値の型の検査は行います。

- IDの `0` も有効です。取得の成否は戻り値の `bool` で判定してください。
- IDは取得したエンジン専用です。他のエンジンへ渡さないでください。
- コンパイルを再実行した場合、成功・失敗にかかわらずIDを再取得してください。
- 古いIDや別エンジンのIDでも数値が既存関数と一致すると呼び出せてしまいます。自動的な失効検査はありません。
- ID自体の解放は不要です。

名前を直接指定する `call_function_value()` も引き続き使用できます。

### GLMの行列を渡して結果を取得する

`value_create_matrix_f32()` は `float`、`value_create_matrix_f64()` は `double` の
連続した列優先バッファを受け取ります。`value_copy_matrix_f32()` / `f64()` で
結果を呼び出し側のバッファへ書き戻せます。

以下は、GLMを使用するC++の呼び出し例です。GLMは呼び出し側で用意してください。
Musubu側では受け取った行列の全成分を2倍にします。

```cpp
#include "musubu.h"
#include <glm/glm.hpp>
#include <glm/gtc/type_ptr.hpp>
#include <cstdio>
#include <memory>

using EngineOwner = std::unique_ptr<MusubuEngine, decltype(&uninit)>;
using ValueOwner = std::unique_ptr<MusubuValueHandle, decltype(&value_free)>;

void print_error(const MusubuEngine* engine, bool compilation) {
    size_t length = 0;
    const char* message = compilation
        ? get_compile_error(engine, &length)
        : get_runtime_error(engine, &length);
    if (message != nullptr) {
        // エラー文字列はNUL終端されていないため、長さを指定して出力します。
        std::fwrite(message, sizeof(char), length, stderr);
        std::fputc('\n', stderr);
    }
}

bool transform_matrix(MusubuEngine* engine, size_t functionId,
                      const glm::mat4& input, glm::mat4& output) {
    constexpr size_t dimension = 4;
    static_assert(sizeof(glm::mat4) ==
                  dimension * dimension * sizeof(float),
                  "A packed column-major matrix is required");

    MusubuValueHandle* rawArgument = nullptr;
    if (!value_create_matrix_f32(engine, glm::value_ptr(input),
                                 dimension, dimension, &rawArgument)) {
        return false;
    }
    ValueOwner argument(rawArgument, &value_free);
    const MusubuValueHandle* arguments[] = {argument.get()};
    constexpr size_t argumentCount = 1;

    MusubuValueHandle* rawResult = nullptr;
    if (!call_function_value_by_id(engine, functionId,
                                    arguments, argumentCount, &rawResult)) {
        return false;
    }
    ValueOwner result(rawResult, &value_free);
    return value_copy_matrix_f32(engine, result.get(), glm::value_ptr(output),
                                 dimension, dimension);
}

int main() {
    MusubuEngine* rawEngine = nullptr;
    if (!init(&rawEngine)) {
        return 1;
    }
    EngineOwner engine(rawEngine, &uninit);
    constexpr char source[] = R"(
        fn transform(m: mat4x4) -> mat4x4 {
            m * 2.0
        }
    )";
    if (!compile(engine.get(), source, sizeof(source) - 1)) {
        print_error(engine.get(), true);
        return 1;
    }

    constexpr char functionName[] = "transform";
    size_t functionId = 0;
    if (!get_function_id(engine.get(), functionName,
                         sizeof(functionName) - 1, &functionId)) {
        print_error(engine.get(), false);
        return 1;
    }

    const glm::mat4 input(1.0f);
    glm::mat4 output(0.0f);
    // 以降も同じfunctionIdでtransform_matrixを繰り返し呼び出せます。
    if (!transform_matrix(engine.get(), functionId, input, output)) {
        print_error(engine.get(), false);
        return 1;
    }
    // outputは対角成分が2、それ以外が0の行列になります。
    return 0;
}
```

バッファの成分位置は `data[column * rows + row]` です。
寸法引数は**列数、行数**の順で、Musubuの `mat2x3` は2列・3行を表します。
引数・戻り値とも、行列の寸法と成分型が正確に一致する必要があります。
`mat4x4` は `f32`、`mat4x4f64` は `f64` の行列です。

列間にパディングがある配置には対応していません。該当するGLMのアラインメント設定などを
使用する場合は、パディングのない列優先バッファへ詰め替えてください。
入力は内部へコピーされるため、呼び出し後に元の行列を変更してもハンドルの値は変わりません。
入力が変わる場合はハンドルを作り直してください。ゼロコピーのAPIではありません。

### 対応型とエラー処理

| API | 引数・戻り値 |
| --- | --- |
| `call_function` | `MusubuValue` による整数・浮動小数点・bool・unit |
| `call_function_value` / `call_function_value_by_id` | ハンドルによる上記の型・vec・mat・enum |

構造体・タプル値・配列・文字列のFFI受け渡しは未対応です。
enumも、いずれかのバリアントのフィールドに未対応型が含まれる場合は受け渡せません。
数値型の暗黙変換は行いません。

コンパイル失敗時は `get_compile_error()`、関数ID取得・実行・値の操作の失敗時は
`get_runtime_error()` で詳細を取得します。ファイル名付きのコンパイルエラーが必要なら
`compile_with_filename()` を使って相対パスを指定してください。
診断文字列は借用したUTF-8バイト列で、解放してはいけません。
特に実行時診断は、次にエンジンを変更するAPIを呼ぶ前に読み出すかコピーしてください。

関数呼び出しで取得した値ハンドルは入力とは独立しています。
成功した値の生成・関数呼び出し・`value_get_element()` ごとに、返されたハンドルを
一度だけ解放してください。同じエンジンへのAPI呼び出しは同時に実行しないでください。
