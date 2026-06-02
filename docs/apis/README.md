# API リファレンス

Imagineration が提供する HTTP API の仕様です。各エンドポイントの詳細は個別のドキュメントを参照してください。

## 共通事項

### ベース URL

サーバーは設定ファイル (`imagineration.toml`) の `[server]` セクションで指定したホスト・ポートで待ち受けます (既定値 `127.0.0.1:3000`)。

画像生成バックエンドには [diffusion-rs](https://github.com/newfla/diffusion-rs) ([stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) の Rust バインディング) を使用します。モデルファイルの探索ルートとなるディレクトリ構成は [directory-structure.md](../directory-structure.md) を参照してください。

### レスポンス形式

- 正常系のレスポンスボディは特記がない限り JSON (`application/json`) です。
- 画像本体を返すエンドポイントのみ `image/png` を返します。

### エラーレスポンス

エラー時は対応する HTTP ステータスコードとともに、以下の形式の JSON を返します。

```json
{
  "error": {
    "message": "エラー内容を説明するメッセージ"
  }
}
```

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | リクエストの内容が不正 (不正なクエリ・ボディ・パスパラメータなど) |
| `404 Not Found` | 指定したリソースが存在しない |
| `500 Internal Server Error` | サーバー内部のエラー (I/O・JSON・PNG エンコードなど) |

## エンドポイント一覧

| メソッド | パス | 説明 | ドキュメント |
| --- | --- | --- | --- |
| `GET` | `/v1/models` | モデルリストを取得する | [get-models.md](./get-models.md) |
| `POST` | `/v1/images:generate` | 画像生成を実行する | [post-images-generate.md](./post-images-generate.md) |
| `GET` | `/v1/images` | 生成済み画像のリストを取得する | [get-images.md](./get-images.md) |
| `GET` | `/v1/images/{image_id}` | 画像本体 (PNG) を取得する | [get-image.md](./get-image.md) |
| `GET` | `/v1/images/{image_id}/metadata` | 画像のメタデータを取得する | [get-image-metadata.md](./get-image-metadata.md) |
