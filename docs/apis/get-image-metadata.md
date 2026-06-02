# GET `/v1/images/{image_id}/metadata`

画像のプロンプト等の入力情報 (メタデータ) を取得します。

[POST `/v1/images:generate`](./post-images-generate.md) で保存された `images_dir/{image_id}.json` の内容を返します。

## リクエスト

### パスパラメータ

| 名前 | 型 | 説明 |
| --- | --- | --- |
| `image_id` | string (UUID) | 取得する画像の ID。UUID 形式である必要があります。 |

## レスポンス

### `200 OK`

```json
{
  "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "created_at": "2026-05-29T12:00:00Z",
  "request": {
    "model": "example.safetensors",
    "prompt": "a cat",
    "steps": 20,
    "width": 512,
    "height": 512
  },
  "input_assets": [
    {
      "json_pointer": "/input_image",
      "mime_type": "image/png",
      "size_bytes": 5
    }
  ],
  "output": {
    "mime_type": "image/png",
    "width": 512,
    "height": 512,
    "image_path": "f47ac10b-58cc-4372-a567-0e02b2c3d479.png"
  }
}
```

#### フィールド

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `id` | string (UUID) | 画像 ID。 |
| `created_at` | string (RFC 3339) | 生成日時 (UTC)。 |
| `request` | object | 生成時のリクエストボディそのまま。 |
| `input_assets` | array | リクエストに含まれていた入力アセット (data URL) の情報。 |
| `input_assets[].json_pointer` | string | リクエストボディ内でアセットが存在した位置を示す [JSON Pointer](https://datatracker.ietf.org/doc/html/rfc6901)。 |
| `input_assets[].mime_type` | string | data URL から抽出した MIME タイプ。 |
| `input_assets[].size_bytes` | integer | デコード後のバイト数。 |
| `output` | object | 出力画像の情報。 |
| `output.mime_type` | string | 出力画像の MIME タイプ (`image/png`)。 |
| `output.width` | integer | 出力画像の幅 (ピクセル)。 |
| `output.height` | integer | 出力画像の高さ (ピクセル)。 |
| `output.image_path` | string | `images_dir` 内の画像ファイル名。 |

### エラー

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | `image_id` が UUID 形式でない。 |
| `404 Not Found` | 指定した ID のメタデータが存在しない。 |
| `500 Internal Server Error` | ファイル読み込み・JSON パース時のエラー。 |

## 例

```bash
curl http://127.0.0.1:3000/v1/images/f47ac10b-58cc-4372-a567-0e02b2c3d479/metadata
```
