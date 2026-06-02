# Imagineration

Imagineration は、画像生成を行う HTTP API サーバーです。

画像生成バックエンドには [diffusion-rs](https://github.com/newfla/diffusion-rs) を使用します。生成された PNG
と、生成時の入力情報を含む JSON メタデータはデフォルトで `data/images/` に保存されます。

## 必要なもの

- Rust 2024 edition に対応した Rust toolchain
- 画像生成に使用するモデルファイル

モデルファイルは、既定では `models/` 配下に配置します。ディレクトリ構成は ComfyUI の `models`
ディレクトリと同様です。詳細は [docs/directory-structure.md](./docs/directory-structure.md) を参照してください。

## セットアップ

設定は [imagineration.toml](./imagineration.toml) で管理します。

```toml
[server]
host = "127.0.0.1"
port = 3000

[paths]
models_dir = "models"
images_dir = "data/images"
```

## 起動

```sh
cargo run -- --config imagineration.toml
```

既定の設定では `http://127.0.0.1:3000` で待ち受けます。

### フロントエンド

フロントエンドは SvelteKit で実装したシングルページアプリケーションで、API を呼び出して画像生成を行う簡単な UI を提供します。
使用には `frontend` feature を有効にしてビルドする必要があります。フロントエンドを有効にしたバイナリを起動すると、APIと同じホストとポートでフロントエンドも配信されるようになります。

### ハードウェアアクセラレーション

ハードウェアアクセラレータを使用する場合は該当するfeatureを有効にする必要があります。
デフォルトでは何も有効にはなっておらず、CPU推論が行われます。

+ `vulkan`: Vulkanをバックエンドとして使用できるようにします
+ `cuda`: NVIDIA CUDAをバックエンドとして使用できるようにします
+ `metal`: Apple SiliconのMetalをバックエンドとして使用できるようにします
+ `hip`: AMD HIPをバックエンドとして使用できるようにします

`frontend` featureを有効にしたバイナリでは、設定ファイルの `[frontend] enabled` で配信の有効・無効を切り替えられます。
featureを有効にせずにビルドした場合、`enabled = true` でもフロントエンドは配信されません。

フロントエンドを更新する場合は `npm` ではなく `pnpm` を使用します。依存バージョンはminimum release
age設定でも解決しやすいように固定しています。

```sh
pnpm --dir imagineration-frontend/web install
pnpm --dir imagineration-frontend/web build
cargo run --features frontend -- --config imagineration.toml
```

## API

API の詳細は [docs/apis/README.md](./docs/apis/README.md) を参照してください。

| メソッド   | パス                               | 説明              |
|--------|----------------------------------|-----------------|
| `GET`  | `/v1/models`                     | モデルリストを取得する     |
| `POST` | `/v1/images:generate`            | 画像生成を実行する       |
| `GET`  | `/v1/images`                     | 生成済み画像のリストを取得する |
| `GET`  | `/v1/images/{image_id}`          | 画像本体を取得する       |
| `GET`  | `/v1/images/{image_id}/metadata` | 画像のメタデータを取得する   |

生成リクエストの例:

```sh
curl -X POST http://127.0.0.1:3000/v1/images:generate \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "example.safetensors",
    "prompt": "a cat sitting on a chair",
    "negative_prompt": "blurry, low quality",
    "steps": 20,
    "cfg_scale": 7.0,
    "width": 768,
    "height": 512
  }'
```

`model` にファイル名のみを指定した場合は、`models/checkpoints/` 配下から探索されます。モデル指定、プリセット、img2img、ControlNet、LoRA
などの詳細は [POST `/v1/images:generate`](./docs/apis/post-images-generate.md) を参照してください。

## ドキュメント

- [docs/apis/README.md](./docs/apis/README.md): API リファレンス
- [docs/directory-structure.md](./docs/directory-structure.md): モデル・画像ディレクトリ構成
