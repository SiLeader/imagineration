# API リファレンス

Imagineration が提供する HTTP API の仕様です。各エンドポイントの詳細は個別のドキュメントを参照してください。

## 共通事項

### ベース URL

サーバーは設定ファイル (`imagineration.toml`) の `[server]` セクションで指定したホスト・ポートで待ち受けます (既定値 `127.0.0.1:3000`)。

画像生成バックエンドには [diffusion-rs](https://github.com/newfla/diffusion-rs) ([stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) の Rust バインディング) を使用します。モデルファイルの探索ルートとなるディレクトリ構成は [directory-structure.md](../directory-structure.md) を参照してください。

### 認証

`/v1` API はトークン認証に対応しています。クライアントは `Authorization: Bearer <token>` ヘッダーでトークンを提示します。設定ファイル (`imagineration.toml`) の `[auth]` セクションで複数の方式を設定でき、いずれかを満たせば認証成功です (OR 評価)。どれも未設定 (既定) の場合、認証は無効になり、すべてのリクエストを許可します。

| 方式 | 設定 | トークンの取得方法 | 検証 |
| --- | --- | --- | --- |
| 固定トークン | `static_tokens` | 事前共有 | 列挙した値と完全一致 |
| ユーザー (静的トークン) | `[[auth.users]]` | `POST /v1/auth/login` がユーザーの `token` を返却 | `token` と完全一致 |
| ローカル発行 JWT | `[[auth.users]]` + `[auth.issuer]` | `POST /v1/auth/login` が HS256 JWT を発行 | サーバー自身が署名・有効期限を検証 |
| 外部 IdP の JWT (OIDC) | `[auth.jwt]` | 外部 IdP (例: Google) が発行 | JWKS で署名・有効期限・`iss`・`aud` を検証 |

- ユーザー認証: `[[auth.users]]` に `username` と、`password` (平文) または `password_sha256` (SHA-256 の hex) を設定します。`[auth.issuer]` を設定しない場合はログイン時にユーザーの `token` を返し、設定した場合は HS256 で署名した JWT を発行します。
- 外部 IdP JWT: `[auth.jwt]` の JWKS は `jwks` (インライン)、`jwks_path` (ファイル)、`jwks_uri` (リモート取得) のいずれかで与えます。`jwks_uri` を使うと JWK がローカルになくてもよく、未知の `kid` を提示されたときに再取得します。

認証が有効な状態でトークンが欠落または不正な場合は `401 Unauthorized` を返します (レスポンスに `WWW-Authenticate: Bearer` ヘッダーを付与)。フロントエンド SPA とそのアセット、`POST /v1/auth/login`、`GET /v1/capabilities` は認証対象外です。設定例は `imagineration.toml` の `[auth]` セクションを参照してください。

### ユーザー定義プリセット

`presets` ビルド機能 (cargo feature) を有効にしてビルドした場合のみ、ユーザーごとに生成設定 (モデル一式・LoRA・VAE・プロンプト・ステップ数・CFG・画像サイズなど) を保存する `/v1/presets` API が有効になります。プリセットは認証済みユーザー (subject) ごとに分離されます。保存先は `[presets]` の `backend` で選択でき、`memory` (非永続)、`file` (JSON)、`sqlite` (`presets-sqlite` 機能が必要) に対応します。

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

| メソッド | パス | 説明 | 認証 | ドキュメント |
| --- | --- | --- | --- | --- |
| `POST` | `/v1/auth/login` | ユーザー名・パスワードでトークンを取得する | 不要 | [post-auth-login.md](./post-auth-login.md) |
| `GET` | `/v1/capabilities` | 有効な機能 (認証・プリセット) を取得する | 不要 | [get-capabilities.md](./get-capabilities.md) |
| `GET` | `/v1/models` | モデルリストを取得する | 必要 | [get-models.md](./get-models.md) |
| `POST` | `/v1/images:generate` | 画像生成を実行する | 必要 | [post-images-generate.md](./post-images-generate.md) |
| `GET` | `/v1/images` | 生成済み画像のリストを取得する | 必要 | [get-images.md](./get-images.md) |
| `GET` | `/v1/images/{image_id}` | 画像本体 (PNG) を取得する | 必要 | [get-image.md](./get-image.md) |
| `GET` | `/v1/images/{image_id}/metadata` | 画像のメタデータを取得する | 必要 | [get-image-metadata.md](./get-image-metadata.md) |
| `GET` | `/v1/presets` | ユーザー定義プリセットの一覧を取得する | 必要 | [presets.md](./presets.md) |
| `POST` | `/v1/presets` | プリセットを作成する | 必要 | [presets.md](./presets.md) |
| `GET` | `/v1/presets/{id}` | プリセットを取得する | 必要 | [presets.md](./presets.md) |
| `PUT` | `/v1/presets/{id}` | プリセットを置き換える | 必要 | [presets.md](./presets.md) |
| `DELETE` | `/v1/presets/{id}` | プリセットを削除する | 必要 | [presets.md](./presets.md) |

> `/v1/presets` 系のエンドポイントは `presets` ビルド機能を有効にした場合のみ存在します。
