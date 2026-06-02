# GET `/v1/models`

モデルディレクトリを走査し、利用可能なモデルの一覧を取得します。

モデルのディレクトリ構造は ComfyUI の `models` ディレクトリと同様です。走査対象のルートは設定ファイルの `[paths] models_dir` (既定値 `models`) です。ディレクトリ構成の詳細は [directory-structure.md](../directory-structure.md) を参照してください。

## リクエスト

### クエリパラメータ

| 名前 | 型 | 必須 | 説明 |
| --- | --- | --- | --- |
| `type` | string | 任意 | モデルタイプ (ComfyUI のフォルダ名) で絞り込みます。例: `checkpoints`, `diffusion_models`, `text_encoders`, `vae`, `loras` など。 |

- `type` を指定した場合は、`models_dir/{type}` ディレクトリのみを走査します。
- `type` を省略した場合は、既知のモデルタイプと `models_dir` 直下に存在する全サブディレクトリを走査します。
  - 既知のモデルタイプ: `checkpoints`, `diffusion_models`, `text_encoders`, `vae`, `loras`, `embeddings`, `controlnet`, `clip_vision`, `upscale_models`, `photomaker`
- `type` にパス区切り文字 (`/`, `\`) や `.` / `..` を含めることはできません (指定すると `400 Bad Request`)。

各モデルタイプのディレクトリ配下はサブディレクトリも含めて再帰的に走査され、見つかったファイルがモデルとして列挙されます。

## レスポンス

### `200 OK`

```json
{
  "models": [
    {
      "name": "example.safetensors",
      "type": "checkpoints",
      "path": "checkpoints/example.safetensors",
      "size_bytes": 5,
      "modified_unix_secs": 1716960000
    }
  ]
}
```

#### フィールド

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `models` | array | モデル情報の配列。`type` 昇順、続いて `name` 昇順でソートされます。 |
| `models[].name` | string | モデルタイプのディレクトリからの相対パス (区切りは `/`)。 |
| `models[].type` | string | モデルタイプ (ディレクトリ名)。 |
| `models[].path` | string | `models_dir` からの相対パス (区切りは `/`)。 |
| `models[].size_bytes` | integer | ファイルサイズ (バイト)。 |
| `models[].modified_unix_secs` | integer \| null | 最終更新時刻 (Unix 秒)。取得できない場合は `null`。 |

`models_dir` が存在しない場合や、対象のモデルタイプのディレクトリが存在しない場合は、空の配列を返します。

### エラー

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | `type` にパス区切りや `.` / `..` が含まれる。 |
| `500 Internal Server Error` | ディレクトリ走査中の I/O エラー。 |

## 例

```bash
# 全モデルを取得
curl http://127.0.0.1:3000/v1/models

# checkpoints のみ取得
curl 'http://127.0.0.1:3000/v1/models?type=checkpoints'
```
