# GET `/v1/capabilities`

サーバーで有効になっている機能を返します。フロントエンドが、ログイン画面を出すべきか、プリセット UI を表示すべきかなどを判断するために使います。このエンドポイントは認証不要です。

## リクエスト

パラメータはありません。

## レスポンス

### `200 OK`

```json
{
  "auth": {
    "required": true,
    "login": true,
    "issues_jwt": false
  },
  "presets": true
}
```

#### フィールド

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `auth.required` | boolean | `/v1` API にトークンが必要かどうか (認証方式が 1 つ以上有効か)。 |
| `auth.login` | boolean | `POST /v1/auth/login` でユーザー名・パスワードからトークンを取得できるか (`[[auth.users]]` が設定されているか)。 |
| `auth.issues_jwt` | boolean | ログイン成功時に JWT を発行するか (`true`)、静的トークンを返すか (`false`)。 |
| `presets` | boolean | ユーザー定義プリセット API が有効か (`presets` ビルド機能 + `[presets].enabled`)。 |

## 例

```bash
curl http://127.0.0.1:3000/v1/capabilities
```
