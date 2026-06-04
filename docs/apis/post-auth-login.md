# POST `/v1/auth/login`

ユーザー名・パスワードを検証し、ベアラートークンを発行します。このエンドポイントは認証不要です。

発行されるトークンの種類は設定に依存します。

- `[auth.issuer]` を設定していない場合: 対象ユーザーの静的トークン (`[[auth.users]].token`) を返します。
- `[auth.issuer]` を設定している場合: HS256 で署名した JWT を発行し、有効期限 (`expires_in`) を返します。

ユーザーは設定ファイル (`imagineration.toml`) の `[[auth.users]]` で定義します。

## リクエスト

### ボディ (`application/json`)

| フィールド | 型 | 必須 | 説明 |
| --- | --- | --- | --- |
| `username` | string | 必須 | ユーザー名。 |
| `password` | string | 必須 | パスワード。 |

```json
{
  "username": "alice",
  "password": "change-me"
}
```

## レスポンス

### `200 OK`

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600
}
```

#### フィールド

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `access_token` | string | 以降の API 呼び出しで `Authorization: Bearer <access_token>` として提示するトークン。 |
| `token_type` | string | 常に `Bearer`。 |
| `expires_in` | integer \| 省略 | JWT 発行時のみ含まれる有効期間 (秒)。静的トークンの場合は省略されます。 |

### エラー

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | ボディが不正な JSON、または必須フィールドが欠落。 |
| `401 Unauthorized` | ユーザー名またはパスワードが不正。 |
| `404 Not Found` | ユーザーが 1 件も設定されていない (ログイン未構成)。 |
| `500 Internal Server Error` | トークンを生成できなかった。 |

## 例

```bash
curl -X POST http://127.0.0.1:3000/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"change-me"}'
```
