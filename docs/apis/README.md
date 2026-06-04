# API リファレンス

Imagineration が提供する HTTP API の仕様です。各エンドポイントの詳細は個別のドキュメントを参照してください。

## 共通事項

### ベース URL

サーバーは設定ファイル (`imagineration.toml`) の `[server]` セクションで指定したホスト・ポートで待ち受けます (既定値 `127.0.0.1:3000`)。

画像生成バックエンドには [diffusion-rs](https://github.com/newfla/diffusion-rs) ([stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) の Rust バインディング) を使用します。モデルファイルの探索ルートとなるディレクトリ構成は [directory-structure.md](../directory-structure.md) を参照してください。

### 認証

`/v1` API はトークン認証に対応しています。クライアントは `Authorization: Bearer <token>` ヘッダーでトークンを提示します。設定ファイル (`imagineration.toml`) の `[auth]` セクションで、固定トークンと OIDC の JWT の 2 方式を設定でき、両者は OR で評価されます。

| `[auth]` の設定 | 挙動 |
| --- | --- |
| 固定トークンと JWT の両方を設定 | いずれかを満たせば認証成功 |
| どちらか一方のみ設定 | 設定した方式のみ有効 |
| どちらも未設定 (既定) | 認証は無効。すべてのリクエストを許可 |

- 固定トークン: `static_tokens` に列挙した値と完全一致するトークンを受理します。
- OIDC JWT: `[auth.jwt]` に検証鍵 (JWKS) と `issuer`・`audiences`・`algorithms` を設定し、署名・有効期限・`iss`・`aud` を検証します。JWKS は `jwks` (インライン) もしくは `jwks_path` (ファイル) で与えます。

認証が有効な状態でトークンが欠落または不正な場合は `401 Unauthorized` を返します (レスポンスに `WWW-Authenticate: Bearer` ヘッダーを付与)。フロントエンド SPA とそのアセットは認証対象外です。設定例は `imagineration.toml` の `[auth]` セクションを参照してください。

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
| `401 Unauthorized` | 認証が有効な状態で、トークンが欠落または不正 |
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
