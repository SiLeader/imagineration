# `/v1/presets` (ユーザー定義プリセット)

ユーザーごとに生成設定 (モデル一式・LoRA・VAE・プロンプト・ステップ数・CFG・画像サイズなど) を保存・再利用するための CRUD API です。

> これらのエンドポイントは `presets` ビルド機能 (cargo feature) を有効にし、かつ `[presets].enabled = true` の場合のみ存在します。無効な場合は `404 Not Found` になります。

すべてのエンドポイントは認証が必要です。プリセットは認証済みユーザー (subject) ごとに分離され、他ユーザーのプリセットは参照・更新・削除できません。保存先は `[presets].backend` (`memory` / `file` / `sqlite`) で選択します。

## プリセットオブジェクト

```json
{
  "id": "0190f3a1-...-...",
  "user": "alice",
  "name": "ポートレート",
  "content": {
    "mode": "checkpoint",
    "model": "example.safetensors",
    "prompt": "a portrait of a cat",
    "negative_prompt": "blurry",
    "loras": [{ "file_name": "detail-tweaker.safetensors", "weight": 0.8 }],
    "vae": "vae.safetensors",
    "width": 768,
    "height": 768,
    "steps": 24,
    "cfg_scale": 7,
    "guidance": 3.5,
    "sampling_method": "dpmpp2m",
    "scheduler": "karras"
  },
  "created_at": "2026-06-04T12:00:00Z",
  "updated_at": "2026-06-04T12:00:00Z"
}
```

| フィールド | 型 | 説明 |
| --- | --- | --- |
| `id` | string (UUID) | プリセット ID (サーバーが採番)。 |
| `user` | string | 所有ユーザー (認証済み subject)。 |
| `name` | string | 表示名 (空文字は不可)。 |
| `content` | object | 生成設定。`/v1/images:generate` のリクエストボディと同じ形のフラットなオブジェクトで、`mode` (UI ヒント) を含みます。明示的に定義されていないキーもそのまま保存されます。 |
| `created_at` / `updated_at` | string (RFC 3339) | 作成・更新時刻。 |

`content` の主なキー: `mode`, `model`, `diffusion_model`, `text_encoders`, `vae`, `preset`, `preset_weight_type`, `loras`, `lora_apply_mode`, `prompt`, `negative_prompt`, `width`, `height`, `steps`, `cfg_scale`, `guidance`, `seed`, `batch_count`, `sampling_method`, `scheduler`。すべて任意です。

## エンドポイント

### `GET /v1/presets`

認証ユーザーのプリセット一覧を、更新日時の新しい順に返します。

```json
{ "presets": [ { "id": "...", "name": "...", "content": { } } ] }
```

### `POST /v1/presets`

プリセットを作成します。成功時は `201 Created` とプリセットオブジェクトを返します。

ボディ:

```json
{ "name": "ポートレート", "content": { "prompt": "a cat", "steps": 24 } }
```

### `GET /v1/presets/{id}`

ID を指定してプリセットを取得します。存在しない (または他ユーザーの) 場合は `404`。

### `PUT /v1/presets/{id}`

プリセットを置き換えます。`created_at` は維持され、`updated_at` が更新されます。存在しない場合は `404`。ボディは `POST` と同形式です。

### `DELETE /v1/presets/{id}`

プリセットを削除します。成功時は `204 No Content`。存在しない場合は `404`。

## エラー

| ステータス | 発生条件 |
| --- | --- |
| `400 Bad Request` | ボディが不正な JSON。 |
| `401 Unauthorized` | 認証が必要だがトークンが欠落・不正。 |
| `404 Not Found` | プリセットが存在しない、または `presets` 機能が無効。 |
| `422 Unprocessable Entity` | `name` が空。 |
| `500 Internal Server Error` | ストレージのエラー。 |

## 例

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:3000/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"change-me"}' | jq -r .access_token)

# 作成
curl -X POST http://127.0.0.1:3000/v1/presets \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"name":"portrait","content":{"prompt":"a cat","steps":24}}'

# 一覧
curl http://127.0.0.1:3000/v1/presets -H "Authorization: Bearer $TOKEN"
```
