# Musubu

自作アプリケーション用の組み込み言語

## MSVC

```cpp
/**
 * code_ptr: ソースコードの先頭ポインタ 文字列フォーマットはUTF-8
 * len: 文字列のバイト数(文字数ではない)
 */
extern "C" __declspec(dllimport) bool compile(const char* code_ptr, size_t len);

// 削除予定
// matrix_ptr: floatの4x4行列 float[16]である必要がある
extern "C" __declspec(dllimport) bool run_script(float* matrix_ptr);
```
