# Musubu

自作アプリケーション用の組み込み言語

## 開発環境と検証

開発・品質検査の基準はRust **1.88.0**です。`rust-toolchain.toml` で固定しています。
CIでは固定版のWindows/Linux・debug/releaseに加え、最新stableとの互換性も別ジョブで検証します。

```sh
cargo test --workspace --locked
cargo build --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

依存を取得済みなら `--offline` を追加して検証できます。
外部C++クライアントによるDLL接続テストは、MSVCのDeveloper PowerShellで実行します。
スクリプトの実行は環境の署名ポリシーに従ってください。

```powershell
./scripts/test-ffi.ps1 -Profile debug
./scripts/test-ffi.ps1 -Profile release
```

このテストは `cl /std:c++17 /EHsc /W4 /WX` で
[ffi_smoke.cpp](tests/ffi_smoke.cpp) をビルドし、生成した `musubu.dll.lib` へリンクします。
GLMには依存せず、スカラー・ID・関数トークンと列優先の非対称なfloat/double行列を往復検証します。
LinuxのC++共有ライブラリ接続もCIで検証します。

反復FFI呼び出しの時間・割り当て数は、ほかのテストを混ぜずに計測できます。
速度の数値は環境に依存するため、合否判定には使用しません。

```sh
cargo test -p musubu --release --locked ffi_performance -- --ignored --nocapture --test-threads=1
```

## C / C++からの利用（MSVC・GCC・Clang）

公開APIは [musubu.h](compiler/musubu/include/musubu.h) に定義されています。
呼び出し側では、このヘッダをインクルードしてください。
以前の `compile(code, length)` 宣言や `run_script()` ではなく、エンジンを作成して関数を呼び出します。

Rust側は `extern "C"` の共有ライブラリを出力します。C / C++側のコンパイラに
Rust側を合わせる必要はありませんが、OSとCPUアーキテクチャは一致させてください。
Windowsでは公開ヘッダがDLLインポートとCの呼び出し規約を指定します。
エンジンや値ハンドルは、作成したMusubuライブラリのAPIで解放してください。

### Windows

RustのMSVCツールチェーンでビルドします（MSVCのビルドツールが必要です）。

```powershell
cargo build -p musubu --release --target x86_64-pc-windows-msvc
```

以下は `main.c` を呼び出し元とする例です。各コンパイラと必要なツールチェーンは
事前に用意してください。MSVCとMSVCターゲットのClangはインポートライブラリ
`musubu.dll.lib`、MinGW-w64 GCCとGNUターゲットのClangは `musubu.dll` を直接リンクします。

```powershell
# MSVC（Developer PowerShell / Command Prompt）
cl /Icompiler/musubu/include main.c target/x86_64-pc-windows-msvc/release/musubu.dll.lib /Fe:app.exe

# MinGW-w64 GCC（x86_64）
gcc -Icompiler/musubu/include main.c target/x86_64-pc-windows-msvc/release/musubu.dll -o app.exe

# Clang（MSVCターゲット、MSVCのビルド環境）
clang --target=x86_64-pc-windows-msvc -Icompiler/musubu/include main.c target/x86_64-pc-windows-msvc/release/musubu.dll.lib -o app.exe

# Clang（MinGW-w64のヘッダ・ライブラリ・リンカを利用できる環境）
clang --target=x86_64-w64-windows-gnu -Icompiler/musubu/include main.c target/x86_64-pc-windows-msvc/release/musubu.dll -o app.exe

Copy-Item target/x86_64-pc-windows-msvc/release/musubu.dll .
.\app.exe
```

MinGW-w64だけでRust側もビルドする場合は、インストール済みの
`x86_64-pc-windows-gnu` ターゲットを `cargo build` に指定し、上記のDLLのパスを
`target/x86_64-pc-windows-gnu/release/musubu.dll` に変更してください。
C++では `main.cpp` と `g++` / `clang++`（MSVCでは `cl`）を使います。

### Linux

Linux上のRustツールチェーンでビルドし、生成した `libmusubu.so` をリンクします。
以下のコマンドはリポジトリのルートで実行します。

```sh
cargo build -p musubu --release
gcc -Icompiler/musubu/include main.c -Ltarget/release -lmusubu -o app
# Clangを使う場合
clang -Icompiler/musubu/include main.c -Ltarget/release -lmusubu -o app
LD_LIBRARY_PATH="$PWD/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ./app
```

C++では `main.cpp` と `g++` / `clang++` を使います。
`main.c` / `main.cpp` は利用側で用意するソースファイルです。
以下のFFIの呼び出し手順とAPIは、いずれのコンパイラでも共通です。

## FFIからの関数呼び出し

1. `init()` でエンジンを作成します。
2. `compile()` または `compile_with_filename()` でソースをコンパイルします。
3. `get_function_token()` でソース上の関数名からトークンを取得します。
4. `call_function_value_by_token()` で呼び出し、戻り値のハンドルを取得します。
5. 値ハンドルは `value_free()`、エンジンは `uninit()` で解放します。

関数名は `"transform"` のように指定します。`main` や `extern "C"` の定義は不要です。
ソース・関数名・ファイル名はUTF-8で渡し、長さには終端NULを含まない**バイト数**を指定します。

コンパイル成功時はプログラム全体を置換します。新しいソースにない関数は削除されます。
失敗した場合は、直前に成功したコードとそのソース情報を保持します。

### 世代検査付き関数トークン（推奨）

`MusubuFunctionToken` はエンジンとプログラム世代を識別します。
初回に `get_function_token()` で取得し、以降は `call_function_value_by_token()` で
名前検索なしに呼び出せます。別エンジンや成功した再コンパイル以前のトークンは拒否されます。
コンパイルに失敗した場合は既存トークンを引き続き使用できます。
フィールドを直接変更せず、取得した値をそのまま保持してください。解放は不要です。

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

bool transform_matrix(MusubuEngine* engine, const MusubuFunctionToken& function,
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
    if (!call_function_value_by_token(engine, &function,
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
    MusubuFunctionToken function{};
    if (!get_function_token(engine.get(), functionName,
                            sizeof(functionName) - 1, &function)) {
        print_error(engine.get(), false);
        return 1;
    }

    const glm::mat4 input(1.0f);
    glm::mat4 output(0.0f);
    // 再コンパイルに成功するまでは同じトークンを再利用できます。
    if (!transform_matrix(engine.get(), function, input, output)) {
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
| `call_function_value` / `call_function_value_by_id` / `call_function_value_by_token` | ハンドルによる上記の型・vec・mat・enum |

構造体・タプル値・配列・文字列のFFI受け渡しは未対応です。
enumも、いずれかのバリアントのフィールドに未対応型が含まれる場合は受け渡せません。
数値型の暗黙変換は行いません。

コンパイル失敗時は `get_compile_error()`、関数ID取得・実行・値の操作の失敗時は
`get_runtime_error()` で詳細を取得します。ファイル名付きのコンパイルエラーが必要なら
`compile_with_filename()` を使って相対パスを指定してください。
診断文字列は借用したUTF-8バイト列で、解放してはいけません。
特に実行時診断は、次にエンジンを変更するAPIを呼ぶ前に読み出すかコピーしてください。

ソースからコンパイルした関数の実行時エラーには、命令に対応するファイル名・行・列・コードと
呼び出し履歴が付属します。整数のゼロ除算は `DivisionByZero`、演算結果が型に収まらない場合は
`ArithmeticOverflow` として区別します。

### 実行制限

既定では1呼び出しあたり100万命令・256フレーム（入口の関数を含む）まで実行します。
`set_execution_limits(engine, instructions, call_depth)` で正の値に変更できます。
0の指定は失敗し、以前の設定を保持します。設定は再コンパイル後も維持されます。

命令数超過は `InstructionLimitExceeded`、呼び出し深度超過は `StackOverflow` で戻り、
次の呼び出しでは新しい予算で実行します。上限は無限ループ・無限再帰からホストへ制御を戻すためのもので、
総メモリ量や実時間の上限を保証するサンドボックスではありません。

関数呼び出しで取得した値ハンドルは入力とは独立しています。
成功した値の生成・関数呼び出し・`value_get_element()` ごとに、返されたハンドルを
一度だけ解放してください。同じエンジンへのAPI呼び出しは同時に実行しないでください。
