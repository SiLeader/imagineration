# GET `/v1/images`

生成済み画像の一覧を取得します。

`[paths] images_dir` (既定値 `data/images`) 配下の `.json` メタデータファイルを走査して一覧を構築します。

## リクエスト

パラメータはありません。

## レスポンス

### `200 OK`

```json
{
  "images": [
    {
      "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
      "created_at": "2026-05-29T12:00:00Z",
      "mime_type": "image/png",
      "image_url": "/v1/images/f47ac10b-58cc-4372-a567-0e02b2c3d479",
      "metadata_url": "/v1/images/f47ac10b-58cc-4372-a567-0e02b2c3d479/metadata"
    }
  ]
}
```

#### フィールド

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `images` | array | 画像サマリの配列。`created_at` の降順 (新しい順) でソートされます。 |
| `images[].id` | string (UUID) | 画像 ID。 |
| `images[].created_at` | string (RFC 3339) | 生成日時 (UTC)。 |
| `images[].mime_type` | string | MIME タイプ (`image/png`)。 |
| `images[].image_url` | string | 画像本体を取得する相対 URL。 |
| `images[].metadata_url` | string | メタデータを取得する相対 URL。 |

`images_dir` が存在しない場合は、空の配列を返します。

### エラー

| ステータス | 発生条件 |
| --- | --- |
| `500 Internal Server Error` | ディレクトリ走査やメタデータ読み込みの I/O・JSON エラー。 |

## 例

```bash
curl http://127.0.0.1:3000/v1/images
```
