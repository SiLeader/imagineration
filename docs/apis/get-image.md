# GET `/v1/images/{image_id}`

画像本体 (PNG) を返します。

PNG には生成時の入力情報がテキストチャンク (キーワード `imagineration.metadata`) として埋め込まれています。

## リクエスト

### パスパラメータ

| 名前 | 型 | 説明 |
| --- | --- | --- |
| `image_id` | string (UUID) | 取得する画像の ID。UUID 形式である必要があります。 |

## レスポンス

### `200 OK`

画像本体のバイナリを返します。

#### ヘッダー

| ヘッダー | 値 |
| --- | --- |
| `Content-Type` | `image/png` |
| `Cache-Control` | `public, max-age=31536000, immutable` |

### エラー

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | `image_id` が UUID 形式でない。 |
| `404 Not Found` | 指定した ID の画像が存在しない。 |
| `500 Internal Server Error` | ファイル読み込み時の I/O エラー。 |

## 例

```bash
curl http://127.0.0.1:3000/v1/images/f47ac10b-58cc-4372-a567-0e02b2c3d479 \
  --output image.png
```
